//! First independent GTK4 layer-shell taskbar for Melania.
//! This requires a compositor advertising zwlr_layer_shell_v1.
//! Window list comes from compositor IPC; thumbnails still require compositor capture.
mod window_groups;
use gtk::{self, gdk, gio, glib, prelude::*};
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Entry, Image, Label, Orientation, Picture};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use melania_services as services;
use std::{cell::{Cell, RefCell}, fs, path::PathBuf, process::Command, rc::Rc, time::Duration};
use melania_control::{request as control_request, current_workspace, Command as WindowCommand, WindowInfo, SOCKET_ENV};

const APP_ID: &str = "org.melaniadesktop.Panel";
const ICON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png");

fn run(program: &str) {
    if let Err(e)=Command::new(program).spawn() { eprintln!("Melania panel cannot launch {program}: {e}"); }
}
fn label(txt:&str,class:&str)->Label{
    let l=Label::new(Some(txt)); l.set_xalign(0.0); if !class.is_empty() {l.add_css_class(class);} l
}
fn make_surface(app:&Application,kind:Layer,anchors:&[Edge],width:i32,height:i32,margin:i32,namespace:&str,exclusive:i32)->ApplicationWindow {
    let w=ApplicationWindow::builder().application(app).default_width(width).default_height(height).decorated(false).build();
    // gtk4-layer-shell must be initialized before any realization.
    w.init_layer_shell();w.set_namespace(Some(namespace));w.set_layer(kind);
    for &edge in anchors {w.set_anchor(edge,true);}
    if anchors.contains(&Edge::Bottom) {w.set_margin(Edge::Bottom,margin);}
    if anchors.contains(&Edge::Top) {w.set_margin(Edge::Top,margin);}
    if anchors.contains(&Edge::Left) {w.set_margin(Edge::Left,margin);}
    if anchors.contains(&Edge::Right) {w.set_margin(Edge::Right,margin);}
    w.set_exclusive_zone(exclusive);
    // Start, quick settings, notifications and calendar use a keyboard-aware
    // overlay. Escape dismisses the surface without shutting down the panel.
    if exclusive == 0 {
        let escape = gtk::EventControllerKey::new();
        escape.set_propagation_phase(gtk::PropagationPhase::Capture);
        let surface = w.clone();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                surface.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        w.add_controller(escape);
    }
    w
}
fn toggle_surface(w:&ApplicationWindow,other:&ApplicationWindow) {
    other.set_visible(false);
    if w.is_visible() { w.set_visible(false); } else { w.present(); }
}
/// XDG user dirs are normally stored in user-dirs.dirs, not exported as
/// XDG_DOCUMENTS_DIR environment variables. Ask the desktop service first,
/// and fall back to localized existing folders without shell interpolation.
fn xdg_shortcut_directory(kind: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    if kind == "HOME" { return home; }
    if let Ok(output) = Command::new("xdg-user-dir").arg(kind).output() {
        if output.status.success() {
            let location = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !location.is_empty() {
                let path = PathBuf::from(location);
                if path.is_dir() { return path; }
            }
        }
    }
    let candidates: &[&str] = match kind {
        "DOCUMENTS" => &["Documenti", "Documents"],
        "PICTURES" => &["Immagini", "Pictures"],
        "DOWNLOAD" => &["Scaricati", "Download", "Downloads"],
        _ => &[],
    };
    for name in candidates {
        let path = home.join(name);
        if path.is_dir() { return path; }
    }
    home
}

fn icon_row(text: &str, icon_name: &str, class: &str) -> Button {
    let button = Button::new();
    button.add_css_class(class);
    let row = GtkBox::new(Orientation::Horizontal, 10);
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(21);
    row.append(&icon);
    let caption = label(text, "melania-row-text");
    caption.set_hexpand(true);
    row.append(&caption);
    button.set_child(Some(&row));
    button
}

fn start_window(app:&Application)->ApplicationWindow {
    // Inspired by the approved Windows-7-like Melania concept: search, icon
    // list, quick locations and a compact account/session footer.
    let w=make_surface(app,Layer::Overlay,&[Edge::Left,Edge::Bottom],392,480,8,"melania-start",0);
    // Position 8 px from left, exactly above the 52 px taskbar with 8 px gap.
    w.set_margin(Edge::Left,8);
    w.set_margin(Edge::Bottom,60);
    w.set_keyboard_mode(KeyboardMode::OnDemand);
    let root=GtkBox::new(Orientation::Vertical,9);
    root.add_css_class("melania-panel");
    root.add_css_class("melania-start-panel");
    if melania_config::load().unwrap_or_default().high_contrast {
        root.add_css_class("high-contrast");
    }
    let query=Entry::builder().placeholder_text("Cerca applicazioni, cartelle, impostazioni...").build();
    query.add_css_class("melania-entry");
    root.append(&query);
    let columns=GtkBox::new(Orientation::Horizontal,10);
    columns.add_css_class("melania-start-columns");
    let left=GtkBox::new(Orientation::Vertical,5);
    left.set_hexpand(true);
    left.append(&label("PROGRAMMI", "melania-section-label"));
    let scroll=gtk::ScrolledWindow::new();
    scroll.set_vexpand(true); scroll.set_hexpand(true);
    scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
    let apps_box=GtkBox::new(Orientation::Vertical,3);
    apps_box.add_css_class("melania-start-apps");
    // Search-only results remain hidden in the normal classic app list.
    let mut searchable:Vec<(Button,String,bool)>=Vec::new();
    for (name,program,icon) in [
        ("Melania Files", "melania-files", "org.melaniadesktop.Files"),
        ("Impostazioni", "melania-settings", "preferences-system"),
    ] {
        let button=icon_row(name,icon,"melania-action");
        let menu=w.clone();
        button.connect_clicked(move |_| { run(program); menu.set_visible(false); });
        apps_box.append(&button);
        searchable.push((button,name.to_lowercase(),false));
    }
    let mut apps=gio::AppInfo::all().into_iter().filter(|app|app.should_show()).collect::<Vec<_>>();
    apps.sort_by_key(|app|app.name().to_string().to_lowercase());
    for app_info in apps {
        let name=app_info.name().to_string();
        let button=Button::new();button.add_css_class("melania-action");
        let row=GtkBox::new(Orientation::Horizontal,10);
        let icon=app_info.icon().map(|i|Image::from_gicon(&i))
            .unwrap_or_else(||Image::from_icon_name("application-x-executable"));
        icon.set_pixel_size(22);
        row.append(&icon);
        let caption=label(&name,"melania-row-text");caption.set_hexpand(true);row.append(&caption);
        button.set_child(Some(&row));
        let menu=w.clone();
        button.connect_clicked(move |_| {
            match app_info.launch(&[],None::<&gio::AppLaunchContext>) {
                Ok(())=>menu.set_visible(false),Err(e)=>eprintln!("Melania app launch: {e}")
            }
        });
        apps_box.append(&button);
        searchable.push((button,name.to_lowercase(),false));
    }
    // GNOME/COSMIC-inspired unified search without background indexing or
    // filesystem traversal: only apps, known settings sections and XDG folders.
    for (name, page, keywords) in [
        ("Aspetto", "appearance", "tema aero glass colori trasparenza scuro"),
        ("Desktop e finestre", "desktop", "snap anteprime finestre barra raggruppamento"),
        ("Spazi di lavoro", "workspaces", "workspace desktop virtuali"),
        ("Rete e Bluetooth", "network", "wifi wi-fi bluetooth connessione"),
        ("Accessibilità", "accessibility", "contrasto movimento animazioni"),
        ("Scorciatoie", "shortcuts", "tastiera alt tab super e ctrl alt workspace"),
        ("Sistema", "system", "informazioni versione sistema"),
    ] {
        let caption = format!("Impostazioni · {name}");
        let button=icon_row(&caption,"preferences-system","melania-action");
        button.add_css_class("melania-search-result");
        button.set_visible(false);
        let menu=w.clone();
        button.connect_clicked(move |_| {
            if let Err(e)=Command::new("melania-settings").arg(page).spawn() {
                eprintln!("Melania settings search: {e}");
            } else { menu.set_visible(false); }
        });
        searchable.push((button.clone(),format!("{} {}",caption.to_lowercase(),keywords),true));
        apps_box.append(&button);
    }
    for (name, kind, icon_name) in [
        ("Home", "HOME", "user-home"),
        ("Documenti", "DOCUMENTS", "folder-documents"),
        ("Immagini", "PICTURES", "folder-pictures"),
        ("Download", "DOWNLOAD", "folder-download"),
    ] {
        let caption=format!("Cartella · {name}");
        let button=icon_row(&caption,icon_name,"melania-action");
        button.add_css_class("melania-search-result");
        button.set_visible(false);
        let menu=w.clone();
        button.connect_clicked(move |_| {
            let folder=xdg_shortcut_directory(kind);
            if let Err(e)=Command::new("melania-files").arg(folder).spawn() {
                eprintln!("Melania folder search: {e}");
            } else {menu.set_visible(false);}
        });
        searchable.push((button.clone(),format!("{} {}",caption.to_lowercase(),kind.to_lowercase()),true));
        apps_box.append(&button);
    }
    let empty=label("Nessun risultato: applicazioni, cartelle o impostazioni", "melania-sub");
    empty.set_visible(false);apps_box.append(&empty);
    let searchable=Rc::new(searchable);
    let searchable_for_filter=searchable.clone();
    query.connect_changed(move |q| {
        let term=q.text().trim().to_lowercase();
        let mut visible=0;
        for (button,name,search_only) in searchable_for_filter.iter() {
            let matches=if term.is_empty() { !search_only } else { name.contains(&term) };
            button.set_visible(matches);
            if matches {visible+=1;}
        }
        empty.set_visible(visible==0);
    });
    let searchable_for_enter=searchable.clone();
    let entry=query.clone();
    query.connect_activate(move |_| {
        let term=entry.text().trim().to_lowercase();
        if term.is_empty(){return;}
        if let Some((button,_,_))=searchable_for_enter.iter().find(|(_,name,_)|name.contains(&term)) {
            button.emit_clicked();
        }
    });
    scroll.set_child(Some(&apps_box));left.append(&scroll);
    columns.append(&left);
    let right=GtkBox::new(Orientation::Vertical,5);
    right.add_css_class("melania-start-shortcuts");
    right.set_size_request(160,-1);
    right.append(&label("ACCESSI RAPIDI","melania-section-label"));
    let shortcuts = [
        ("Home", "HOME", "user-home"),
        ("Documenti", "DOCUMENTS", "folder-documents"),
        ("Immagini", "PICTURES", "folder-pictures"),
        ("Download", "DOWNLOAD", "folder-download"),
    ];
    for (name,kind,icon_name) in shortcuts {
        let button=icon_row(name,icon_name,"melania-shortcut");
        let menu=w.clone();
        button.connect_clicked(move |_| {
            let path=xdg_shortcut_directory(kind);
            if let Err(e)=Command::new("xdg-open").arg(path).spawn(){eprintln!("Melania shortcut: {e}");}
            menu.set_visible(false);
        });
        right.append(&button);
    }
    right.append(&gtk::Separator::new(Orientation::Horizontal));
    right.append(&label("SISTEMA", "melania-section-label"));
    let computer=icon_row("Computer","computer","melania-shortcut");
    computer.connect_clicked(|_|run("melania-files"));right.append(&computer);
    let settings=icon_row("Impostazioni","preferences-system","melania-shortcut");
    settings.connect_clicked(|_|run("melania-settings"));right.append(&settings);
    columns.append(&right);root.append(&columns);
    let footer=GtkBox::new(Orientation::Horizontal,8);
    footer.add_css_class("melania-start-footer");
    let logo=Image::from_icon_name("melania");logo.set_pixel_size(26);footer.append(&logo);
    let user=label("Melania Desktop", "melania-row-text");
    user.set_hexpand(true);footer.append(&user);
    // Never power down the host from a nested compositor preview.
    let exit=Button::with_label("Esci da Melania");
    exit.add_css_class("melania-session-button");
    // EXIT means stop only our nested Wayland session. It never invokes
    // systemctl poweroff or another host-wide shutdown command.
    let socket=std::env::var_os(SOCKET_ENV).map(PathBuf::from);
    exit.set_sensitive(socket.is_some());
    exit.connect_clicked(move |_| {
        if let Some(socket)=socket.as_ref() {
            if let Err(e)=control_request(socket,WindowCommand::Quit) {
                eprintln!("Melania session exit: {e}");
            }
        }
    });
    footer.append(&exit);
    root.append(&footer);
    w.set_child(Some(&root));w
}

fn binary(parent:&GtkBox,title:&'static str,getter:fn()->services::Result<bool>,setter:fn(bool)->services::Result<()>,status:&Label){
    let b=Button::new();b.add_css_class("melania-action");
    let row=GtkBox::new(Orientation::Horizontal,8);
    let icon=Image::from_icon_name(match title {
        "Wi-Fi"=>"network-wireless", "Bluetooth"=>"bluetooth-active", _=>"preferences-system"
    });
    icon.set_pixel_size(23);
    row.append(&icon);
    let caption=Label::new(Some(title));
    caption.set_xalign(0.0);
    row.append(&caption);
    b.set_child(Some(&row));
    if let Ok(active)=getter(){caption.set_text(&format!("{title}  {}",if active{"● Attivo"}else{"○ Spento"}));}
    else {b.set_sensitive(false);b.set_tooltip_text(Some("Servizio Linux non disponibile"));}
    let feedback=status.clone();
    b.connect_clicked(move |_| {
        match getter().and_then(|state|setter(!state)).and_then(|_|getter()) {
            Ok(active)=>{caption.set_text(&format!("{title}  {}",if active{"● Attivo"}else{"○ Spento"}));feedback.set_text("Impostazione applicata");}
            Err(e)=>feedback.set_text(&format!("{title}: {e}")),
        }
    });parent.append(&b);
}
fn slider(parent:&GtkBox,title:&'static str,get:fn()->services::Result<u8>,set:fn(u8)->services::Result<()>,status:&Label){
    parent.append(&label(title,"melania-sub"));
    let row=GtkBox::new(Orientation::Horizontal,8);
    let value=get().ok();
    let scale=gtk::Scale::with_range(Orientation::Horizontal,0.0,100.0,1.0);
    scale.set_hexpand(true);scale.set_value(value.unwrap_or(50) as f64);scale.set_sensitive(value.is_some());
    let apply=Button::with_label("Applica");apply.set_sensitive(value.is_some());
    let s=scale.clone();let feedback=status.clone();
    apply.connect_clicked(move |_|match set(s.value().round().clamp(0.0,100.0) as u8) {
        Ok(())=>feedback.set_text(&format!("{title} aggiornato")),Err(e)=>feedback.set_text(&e.to_string())
    });
    row.append(&scale);row.append(&apply);parent.append(&row);
}
fn quick_window(app:&Application, notifications:&ApplicationWindow)->ApplicationWindow {
    let w=make_surface(app,Layer::Overlay,&[Edge::Right,Edge::Bottom],350,430,12,"melania-quick",0);
    w.set_margin(Edge::Bottom,65);
    w.set_keyboard_mode(KeyboardMode::OnDemand);
    let root=GtkBox::new(Orientation::Vertical,12);root.add_css_class("melania-panel");
    if melania_config::load().unwrap_or_default().high_contrast {root.add_css_class("high-contrast");}
    let tabs=GtkBox::new(Orientation::Horizontal,6);
    tabs.add_css_class("melania-tabs");
    let notifications_button=Button::with_label("Notifiche");
    notifications_button.add_css_class("melania-tab");
    let current=Button::with_label("Area rapida");
    current.add_css_class("melania-tab-selected");
    current.set_sensitive(false);
    let other=notifications.clone();let myself=w.clone();
    notifications_button.connect_clicked(move |_| {myself.set_visible(false);other.present();});
    tabs.append(&notifications_button);tabs.append(&current);root.append(&tabs);
    root.append(&label("Rete e dispositivi", "melania-sub"));
    let row=GtkBox::new(Orientation::Horizontal,8);
    let feedback=label("Servizi Linux del dispositivo", "melania-status");
    binary(&row,"Wi-Fi",services::wifi_enabled,services::set_wifi,&feedback);
    binary(&row,"Bluetooth",services::bluetooth_enabled,services::set_bluetooth,&feedback);
    row.add_css_class("melania-toggle-row");root.append(&row);
    root.append(&label("Audio e schermo", "melania-sub"));
    slider(&root,"Volume",services::volume,services::set_volume,&feedback);
    slider(&root,"Luminosità",services::brightness,services::set_brightness,&feedback);
    let prefs=Rc::new(RefCell::new(melania_config::load().unwrap_or_default()));
    let dnd=Button::with_label(if prefs.borrow().do_not_disturb {"Non disturbare: attivo"} else {"Non disturbare: spento"});
    let prefs_ref=prefs.clone();let status=feedback.clone();
    dnd.connect_clicked(move |b| {
        let mut p=prefs_ref.borrow_mut();p.do_not_disturb=!p.do_not_disturb;
        b.set_label(if p.do_not_disturb {"Non disturbare: attivo"} else {"Non disturbare: spento"});
        match melania_config::save(&p) {Ok(())=>status.set_text("Preferenza salvata; notifiche non ancora integrate"),Err(e)=>status.set_text(&format!("{e}"))}
    });root.append(&dnd);
    root.append(&feedback);
    let setting=Button::with_label("Apri Impostazioni");setting.connect_clicked(|_|run("melania-settings"));root.append(&setting);
    w.set_child(Some(&root));w
}
fn pins_path()->Option<PathBuf> {
    let dir=std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|PathBuf::from(h).join(".config")))?;
    Some(dir.join("melania").join("pinned-apps"))
}
fn load_pins()->Vec<String> {
    const DEFAULT:[&str;2]=["melania-files","melania-settings"];
    let Some(path)=pins_path() else{return DEFAULT.iter().map(|s|s.to_string()).collect();};
    let text=fs::read_to_string(path).unwrap_or_default();
    let mut ids:Vec<String>=text.lines().filter(|s|DEFAULT.contains(s))
        .map(str::to_string).collect();
    ids.dedup();
    for id in DEFAULT {if !ids.iter().any(|s|s==id){ids.push(id.to_string());}}
    ids
}
fn save_pins(ids:&[String])->std::io::Result<()> {
    let Some(path)=pins_path() else{return Ok(());};
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    fs::write(path,ids.join("\n"))
}
fn app_button(label:&str,command:&'static str)->Button {
    let b=Button::new();b.add_css_class("melania-pin");
    let row=GtkBox::new(Orientation::Horizontal,6);
    let icon=Image::from_icon_name(match command {
        "melania-files"=>"org.melaniadesktop.Files",
        "melania-settings"=>"org.melaniadesktop.Settings",
        _=>"applications-system"
    });
    icon.set_pixel_size(28);
    row.append(&icon);
    b.set_child(Some(&row));
    b.add_css_class("melania-icon-only");
    b.set_tooltip_text(Some(label));
    b.connect_clicked(move |_|run(command));b
}

/// The running-window strip is driven exclusively by compositor state.
/// It never guesses windows from processes or .desktop launcher entries.
fn running_window_strip(pinned_buttons: Rc<RefCell<Vec<(String, Button)>>>) -> GtkBox {
    let box_running = GtkBox::new(Orientation::Horizontal, 4);
    box_running.add_css_class("melania-running");
    let last: Rc<RefCell<Option<Vec<WindowInfo>>>> = Rc::new(RefCell::new(None));
    let settings = melania_config::load().unwrap_or_default();
    let group_enabled = settings.group_windows;
    let use_thumbnails = settings.preview_windows;
    let path = std::env::var_os(SOCKET_ENV).map(PathBuf::from);
    let refresh = {
        let target = box_running.clone();
        move || {
            let result = path.as_ref().map(|p| control_request(p, WindowCommand::List));
            let Ok(windows) = result.unwrap_or_else(||Err(melania_control::ControlError::Protocol("socket non disponibile"))) else {
                if last.borrow().is_some() {
                    for (_, button) in pinned_buttons.borrow().iter() {
                        button.set_visible(true);
                    }
                    while let Some(child) = target.first_child() {target.remove(&child);}
                    *last.borrow_mut() = None;
                }
                return;
            };
            if last.borrow().as_ref() == Some(&windows) {return;}
            // A running Melania application replaces its launcher, rather
            // than showing a duplicate icon elsewhere on the taskbar.
            for (command, button) in pinned_buttons.borrow().iter() {
                let running = windows.iter().any(|w| {
                    if command == "melania-files" {
                        w.app_id.eq_ignore_ascii_case("org.melaniadesktop.Files")
                    } else {
                        w.app_id.eq_ignore_ascii_case("org.melaniadesktop.Settings")
                    }
                });
                button.set_visible(!running);
            }
            while let Some(child) = target.first_child() {target.remove(&child);}
            for group in window_groups::group_windows(&windows, group_enabled) {
                // Grouped applications use a native, keyboard-accessible GTK
                // popover. Each action still addresses one compositor window ID.
                if group.windows.len() > 1 {
                    let first = group.windows.iter().find(|w| w.active)
                        .unwrap_or(&group.windows[0]);
                    let count = group.windows.len();
                    let menu = gtk::MenuButton::new();
                    menu.add_css_class("melania-window");
                    menu.add_css_class("melania-window-group");
                    if group.active() { menu.add_css_class("melania-window-active"); }
                    if group.all_minimized() { menu.add_css_class("melania-window-minimized"); }
                    menu.set_tooltip_text(Some(&format!("{count} finestre — {}", group.app_id)));
                    let entry = gtk::Overlay::new();
                    let icon = Image::from_icon_name(&first.app_id);
                    icon.set_pixel_size(28);
                    entry.set_child(Some(&icon));
                    let count_badge = Label::new(Some(&count.to_string()));
                    count_badge.add_css_class("melania-icon-count");
                    count_badge.set_halign(gtk::Align::End);
                    count_badge.set_valign(gtk::Align::End);
                    entry.add_overlay(&count_badge);
                    menu.add_css_class("melania-icon-only");
                    menu.set_child(Some(&entry));
                    let popover = gtk::Popover::new();
                    popover.add_css_class("melania-group-popover");
                    let list = GtkBox::new(Orientation::Vertical, 5);
                    list.add_css_class("melania-group-list");
                    let mut preview_targets: Vec<(u64,Image,Picture)> = Vec::new();
                    for grouped_window in &group.windows {
                        let id = grouped_window.id;
                        let text = if grouped_window.title.chars().count() > 48 {
                            format!("{}…", grouped_window.title.chars().take(47).collect::<String>())
                        } else { grouped_window.title.clone() };
                        let row = GtkBox::new(Orientation::Horizontal, 6);
                        row.add_css_class("melania-window-preview");
                        let preview_icon = Image::from_icon_name(&grouped_window.app_id);
                        preview_icon.set_pixel_size(38);
                        preview_icon.add_css_class("melania-preview-icon");
                        row.append(&preview_icon);
                        let thumbnail = Picture::new();
                        thumbnail.add_css_class("melania-live-preview");
                        thumbnail.set_size_request(224,140);
                        thumbnail.set_content_fit(gtk::ContentFit::Contain);
                        thumbnail.set_visible(false);
                        row.append(&thumbnail);
                        preview_targets.push((id,preview_icon.clone(),thumbnail));
                        let focus = Button::with_label(&text);
                        focus.add_css_class("melania-action");
                        focus.set_hexpand(true);
                        focus.set_tooltip_text(Some(&grouped_window.title));
                        let action = if grouped_window.active && !grouped_window.minimized {
                            WindowCommand::Minimize(id)
                        } else { WindowCommand::Activate(id) };
                        let socket = path.clone();
                        let flyout = popover.clone();
                        focus.connect_clicked(move |_| {
                            if let Some(endpoint) = socket.as_ref() {
                                if let Err(e) = control_request(endpoint, action) {
                                    eprintln!("Melania grouped taskbar: {e}");
                                }
                            }
                            flyout.popdown();
                        });
                        row.append(&focus);
                        let close = Button::with_label("×");
                        close.add_css_class("melania-close-window");
                        close.set_tooltip_text(Some("Chiudi questa finestra"));
                        let socket = path.clone();
                        let flyout = popover.clone();
                        close.connect_clicked(move |_| {
                            if let Some(endpoint) = socket.as_ref() {
                                if let Err(e) = control_request(endpoint, WindowCommand::Close(id)) {
                                    eprintln!("Melania grouped close: {e}");
                                }
                            }
                            flyout.popdown();
                        });
                        row.append(&close);
                        list.append(&row);
                    }
                    popover.set_child(Some(&list));
                    menu.set_popover(Some(&popover));
                    // Open the real window chooser on hover as well as on click.
                    // Until Smithay exposes thumbnails the preview is an app
                    // icon + window title, never a fabricated screen capture.
                    let hover = gtk::EventControllerMotion::new();
                    let menu_for_hover = menu.clone();
                    let screenshot_endpoint = path.clone();
                    hover.connect_enter(move |_,_,_| {
                        menu_for_hover.popup();
                        // Captures only the selected compositor-owned window
                        // subtree, never screen pixels behind another app.
                        // Six captures max per hover; failure retains icon/title.
                        if !use_thumbnails {return;}
                        let Some(socket)=screenshot_endpoint.as_ref() else {return;};
                        for (id,icon,picture) in preview_targets.iter().take(6) {
                            if let Ok(image_path)=melania_control::request_preview(socket,*id) {
                                picture.set_filename(Some(&image_path));
                                picture.set_visible(true);
                                icon.set_visible(false);
                            }
                        }
                    });
                    menu.add_controller(hover);
                    // Preserve safe hover activation when dragging files over a
                    // group; DnD remains COPY-only and is owned by the client.
                    let hover_target = gtk::DropTarget::new(
                        gdk::FileList::static_type(), gdk::DragAction::COPY
                    );
                    let hovering = Rc::new(Cell::new(false));
                    let entered = hovering.clone();
                    let hover_socket = path.clone();
                    let hover_id = first.id;
                    hover_target.connect_enter(move |_,_,_| {
                        entered.set(true);
                        let still_here = entered.clone();
                        let endpoint = hover_socket.clone();
                        glib::timeout_add_local_once(Duration::from_millis(500), move || {
                            if still_here.get() {
                                if let Some(socket) = endpoint.as_ref() {
                                    let _ = control_request(socket, WindowCommand::Activate(hover_id));
                                }
                            }
                        });
                        gdk::DragAction::COPY
                    });
                    let exited = hovering.clone();
                    hover_target.connect_leave(move |_| exited.set(false));
                    menu.add_controller(hover_target);
                    target.append(&menu);
                    continue;
                }
                let window = &group.windows[0];
                let id = window.id;
                let focus = Button::new();
                focus.add_css_class("melania-window");
                focus.add_css_class("melania-icon-only");
                let window_icon = Image::from_icon_name(&window.app_id);
                window_icon.set_pixel_size(28);
                focus.set_child(Some(&window_icon));
                focus.set_tooltip_text(Some(&format!("{} — {}",window.title,window.app_id)));
                if window.active {focus.add_css_class("melania-window-active");}
                if window.minimized {focus.add_css_class("melania-window-minimized");}
                let action = if window.active && !window.minimized {
                    WindowCommand::Minimize(id)
                } else { WindowCommand::Activate(id) };
                let socket = path.clone();
                focus.connect_clicked(move |_| {
                    if let Some(socket) = socket.as_ref() {
                        if let Err(e) = control_request(socket,action) {eprintln!("Melania taskbar: {e}");}
                    }
                });
                // Hover-to-activate: dragging a local file over an app button
                // requests that its window come forward. The file itself must
                // still be dropped into the *client*, not silently on the bar.
                let file_hover = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
                let hovering = Rc::new(Cell::new(false));
                let entered = hovering.clone();
                let hover_socket = path.clone();
                file_hover.connect_enter(move |_,_,_| {
                    entered.set(true);
                    let still_here = entered.clone();
                    let endpoint = hover_socket.clone();
                    glib::timeout_add_local_once(Duration::from_millis(500),move || {
                        if still_here.get() {
                            if let Some(socket) = endpoint.as_ref() {
                                let _ = control_request(socket,WindowCommand::Activate(id));
                            }
                        }
                    });
                    gdk::DragAction::COPY
                });
                let left = hovering.clone();
                file_hover.connect_leave(move |_|left.set(false));
                focus.add_controller(file_hover);
                // Closing a window remains available through app controls,
                // not as a stray × beside every taskbar icon.
                target.append(&focus);
            }
            *last.borrow_mut()=Some(windows);
        }
    };
    refresh();
    glib::timeout_add_local(Duration::from_millis(500),move ||{
        refresh();glib::ControlFlow::Continue
    });
    box_running
}

/// User-configured workspaces, displayed in the taskbar. State is queried
/// from the compositor rather than guessed from button clicks.
fn workspace_strip() -> GtkBox {
    let row=GtkBox::new(Orientation::Horizontal,2);
    row.add_css_class("melania-workspace-strip");
    let path=std::env::var_os(SOCKET_ENV).map(PathBuf::from);
    let mut buttons=Vec::new();
    let count = melania_config::load().unwrap_or_default().workspace_count;
    let extras = GtkBox::new(Orientation::Vertical, 3);
    extras.add_css_class("melania-workspace-extras");
    for n in 1..=count {
        let b=Button::with_label(&n.to_string());
        b.add_css_class("melania-workspace-button");
        b.set_tooltip_text(Some(&format!("Spazio di lavoro {n}")));
        b.set_sensitive(path.is_some());
        let socket=path.clone();
        b.connect_clicked(move |_| {
            if let Some(socket)=socket.as_ref() {
                if let Err(error)=control_request(socket,WindowCommand::Workspace(n)) {
                    eprintln!("Melania workspace {n}: {error}");
                }
            }
        });
        if n <= 4 { row.append(&b); } else { extras.append(&b); }
        buttons.push(b);
    }
    // At most four workspace buttons occupy the bar. Remaining desktops
    // live in a popover, keeping the 1024px Aero layout usable.
    let overflow = if count > 4 {
        let more = gtk::MenuButton::new();
        more.set_label("Altri ▾");
        more.add_css_class("melania-workspace-overflow");
        more.set_tooltip_text(Some("Altri spazi di lavoro"));
        let popover = gtk::Popover::new();
        popover.add_css_class("melania-group-popover");
        popover.set_child(Some(&extras));
        more.set_popover(Some(&popover));
        row.append(&more);
        Some(more)
    } else { None };
    let refresh=move || {
        if let Some(path)=path.as_ref() {
            if let Ok(active)=current_workspace(path) {
                if let Some(ref more) = overflow {
                    if active > 4 {
                        more.set_label(&format!("{active} ▾"));
                        more.add_css_class("melania-workspace-active");
                    } else {
                        more.set_label("Altri ▾");
                        more.remove_css_class("melania-workspace-active");
                    }
                }
                for (index,button) in buttons.iter().enumerate() {
                    if (index+1) as u8==active {button.add_css_class("melania-workspace-active");}
                    else {button.remove_css_class("melania-workspace-active");}
                }
            }
        }
    };
    refresh();
    glib::timeout_add_local(Duration::from_millis(900),move || {
        refresh();glib::ControlFlow::Continue
    });
    row
}

/// Freedesktop notifications are received by melania-notifyd over D-Bus.
/// This panel reads its atomic per-session snapshot and never trusts unbounded content.
fn notifications_window(app: &Application) -> ApplicationWindow {
    let w=make_surface(app,Layer::Overlay,&[Edge::Right,Edge::Bottom],380,440,12,"melania-notifications",0);
    w.set_margin(Edge::Bottom,65);
    w.set_keyboard_mode(KeyboardMode::OnDemand);
    let root=GtkBox::new(Orientation::Vertical,10);root.add_css_class("melania-panel");
    if melania_config::load().unwrap_or_default().high_contrast {root.add_css_class("high-contrast");}
    let tabs=GtkBox::new(Orientation::Horizontal,6);
    tabs.add_css_class("melania-tabs");
    let current=Button::with_label("Notifiche");
    current.add_css_class("melania-tab-selected");current.set_sensitive(false);
    tabs.append(&current);
    let quick_hint=label("Area rapida  ·  dalla barra", "melania-tab-hint");
    tabs.append(&quick_hint);
    root.append(&tabs);
    let toolbar=GtkBox::new(Orientation::Horizontal,8);
    toolbar.append(&label("Centro notifiche · Melania Desktop", "melania-sub"));
    let spacer=GtkBox::new(Orientation::Horizontal,1);spacer.set_hexpand(true);toolbar.append(&spacer);
    let mark_read=Button::with_label("Segna lette");
    mark_read.add_css_class("melania-action");
    mark_read.set_tooltip_text(Some("Segna come lette senza chiudere le notifiche"));
    mark_read.connect_clicked(|_| {
        if let Err(error)=Command::new("melania-notifyctl").arg("mark-read").spawn() {
            eprintln!("Melania notifications mark read: {error}");
        }
    });
    toolbar.append(&mark_read);
    root.append(&toolbar);
    let scroll=gtk::ScrolledWindow::new();scroll.set_vexpand(true);
    scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
    let entries=GtkBox::new(Orientation::Vertical,8);
    scroll.set_child(Some(&entries));root.append(&scroll);
    let hint=label("Le notifiche arrivano dalle applicazioni Linux tramite D-Bus.","melania-status");
    hint.set_wrap(true);root.append(&hint);
    w.set_child(Some(&root));
    let path=melania_notify_core::snapshot_path().ok();
    let previous=Rc::new(Cell::new(u64::MAX));
    let refresh={
        let entries=entries.clone();let previous=previous.clone();
        move || {
            let result=path.as_ref().and_then(|p|melania_notify_core::read_snapshot(p).ok());
            let Some(snapshot)=result else {return;};
            if previous.get()==snapshot.generation {return;}
            previous.set(snapshot.generation);
            while let Some(child)=entries.first_child(){entries.remove(&child);}
            if snapshot.notifications.is_empty(){
                entries.append(&label("Nessuna notifica", "melania-sub"));
            }else{
                for notice in snapshot.notifications.into_iter().take(30) {
                    let tile=GtkBox::new(Orientation::Vertical,4);
                    tile.add_css_class("melania-notice");
                    let head=GtkBox::new(Orientation::Horizontal,6);
                    head.append(&label(&notice.app,"melania-sub"));
                    let stretch=GtkBox::new(Orientation::Horizontal,1);
                    stretch.set_hexpand(true);head.append(&stretch);
                    let close=Button::with_label("×");
                    close.add_css_class("melania-close-notice");
                    close.set_tooltip_text(Some("Chiudi la notifica"));
                    let id=notice.id;
                    close.connect_clicked(move |_| {
                        if let Err(error)=Command::new("melania-notifyctl")
                            .arg("close").arg(id.to_string()).spawn() {
                            eprintln!("Melania notifications close: {error}");
                        }
                    });
                    head.append(&close);tile.append(&head);
                    if notice.unread {tile.add_css_class("melania-notice-unread");}
                    let title=label(&notice.summary,"melania-notice-title");
                    title.set_wrap(true);tile.append(&title);
                    if !notice.body.is_empty(){
                        let content=label(&notice.body,"melania-notice-body");
                        content.set_wrap(true);content.set_lines(3);tile.append(&content);
                    }
                    entries.append(&tile);
                }
            }
        }
    };
    refresh();
    glib::timeout_add_local(Duration::from_millis(600),move ||{
        refresh();glib::ControlFlow::Continue
    });
    w
}

/// Gregorian month rules live in melania-shell-model for unit tests.
fn render_calendar(grid: &gtk::Grid, heading: &Label, year: i32, month: u32, today: (i32,u32,u32)) {
    while let Some(child)=grid.first_child() {grid.remove(&child);}
    const MONTHS:[&str;12]=["Gennaio","Febbraio","Marzo","Aprile","Maggio","Giugno",
        "Luglio","Agosto","Settembre","Ottobre","Novembre","Dicembre"];
    heading.set_text(&format!("{} {}",MONTHS[(month-1) as usize],year));
    for (column,day) in ["L","M","M","G","V","S","D"].iter().enumerate() {
        let l=label(day,"melania-calendar-weekday");l.set_xalign(0.5);
        grid.attach(&l,column as i32,0,1,1);
    }
    for (index,date) in melania_shell_model::month_grid(year,month).iter().enumerate() {
        let text=date.map(|n|n.to_string()).unwrap_or_default();
        let l=Label::new(Some(&text));l.add_css_class("melania-calendar-day");
        if date.is_some_and(|d| (year,month,d)==today) {l.add_css_class("melania-calendar-today");}
        grid.attach(&l,(index%7) as i32,(index/7+1) as i32,1,1);
    }
}
fn calendar_window(app:&Application)->ApplicationWindow {
    let w=make_surface(app,Layer::Overlay,&[Edge::Right,Edge::Bottom],330,315,12,"melania-calendar",0);
    w.set_margin(Edge::Bottom,58);
    w.set_keyboard_mode(KeyboardMode::OnDemand);
    let root=GtkBox::new(Orientation::Vertical,9);
    root.add_css_class("melania-panel");root.add_css_class("melania-calendar-panel");
    if melania_config::load().unwrap_or_default().high_contrast {root.add_css_class("high-contrast");}
    let now=glib::DateTime::now_local().expect("orologio del sistema");
    let today=(now.year(),now.month() as u32,now.day_of_month() as u32);
    let visible=Rc::new(Cell::new((today.0,today.1)));
    let heading=label("", "melania-heading");
    let nav=GtkBox::new(Orientation::Horizontal,8);
    heading.set_hexpand(true);nav.append(&heading);
    let previous=Button::new();previous.add_css_class("melania-cal-nav");
    let prev_icon=Image::from_icon_name("go-previous-symbolic");prev_icon.set_pixel_size(18);
    previous.set_child(Some(&prev_icon));previous.set_tooltip_text(Some("Mese precedente"));nav.append(&previous);
    let next=Button::new();next.add_css_class("melania-cal-nav");
    let next_icon=Image::from_icon_name("go-next-symbolic");next_icon.set_pixel_size(18);
    next.set_child(Some(&next_icon));next.set_tooltip_text(Some("Mese successivo"));nav.append(&next);
    root.append(&nav);
    let grid=gtk::Grid::new();grid.set_column_homogeneous(true);
    grid.set_row_spacing(3);grid.set_column_spacing(3);root.append(&grid);
    render_calendar(&grid,&heading,today.0,today.1,today);
    for (button,delta) in [(previous,-1),(next,1)] {
        let calendar=grid.clone();let heading=heading.clone();let state=visible.clone();
        button.connect_clicked(move |_| {
            let (mut year,mut month)=state.get();
            if delta==-1 { if month==1 {month=12;year-=1;}else{month-=1;} }
            else if month==12 {month=1;year+=1;}else{month+=1;}
            state.set((year,month));render_calendar(&calendar,&heading,year,month,today);
        });
    }
    root.append(&label("Calendario di Melania  ·  Vista mensile", "melania-sub"));
    w.set_child(Some(&root));w
}

fn taskbar(app:&Application,start:&ApplicationWindow,quick:&ApplicationWindow,notifications:&ApplicationWindow,calendar:&ApplicationWindow)->ApplicationWindow {
    let w=make_surface(app,Layer::Top,&[Edge::Bottom,Edge::Left,Edge::Right],0,52,0,"melania-panel",52);
    w.set_keyboard_mode(KeyboardMode::None);
    let bar=GtkBox::new(Orientation::Horizontal,8);bar.add_css_class("melania-bar");
    if melania_config::load().unwrap_or_default().high_contrast {
        bar.add_css_class("high-contrast");
    }
    let menu=Button::new();menu.add_css_class("melania-start");
    let logo=Image::from_icon_name("melania");logo.set_pixel_size(31);menu.set_child(Some(&logo));menu.set_tooltip_text(Some("Menu Melania"));
    let sm=start.clone();let qs=quick.clone();let nt=notifications.clone();
    menu.connect_clicked(move |_|{nt.set_visible(false);toggle_surface(&sm,&qs);});bar.append(&menu);
    // App pins are genuine GTK drop targets. Dragging reorders the buttons.
    // A dropped file is NOT silently moved/deleted, and cannot execute arbitrary .desktop files.
    let pinned=GtkBox::new(Orientation::Horizontal,6);
    let pinned_buttons: Rc<RefCell<Vec<(String, Button)>>> = Rc::new(RefCell::new(Vec::new()));
    let pins=load_pins();
    let pinned_order=Rc::new(RefCell::new(pins.clone()));
    for id in pins {
        let (label_name,program):(&str,&'static str)=match id.as_str() {
            "melania-settings"=>("Impostazioni","melania-settings"),
            _=>("File","melania-files"),
        };
        let b=app_button(label_name,program);
        let drag=gtk::DragSource::new();drag.set_actions(gdk::DragAction::MOVE);
        drag.connect_prepare(move |_,_,_|Some(gdk::ContentProvider::for_value(&program.to_value())));
        b.add_controller(drag);
        let drop=gtk::DropTarget::new(String::static_type(),gdk::DragAction::MOVE);
        let order=pinned_order.clone();let area=pinned.clone();let destination=b.clone();
        drop.connect_drop(move |_,value,_,_| {
            let Ok(src)=value.get::<String>() else{return false;};
            if !["melania-files","melania-settings"].contains(&src.as_str()) {return false;}
            let ids_new = {
                let mut ids=order.borrow_mut();
                let Some(from)=ids.iter().position(|v|v==&src) else{return false;};
                let Some(to)=ids.iter().position(|v|v==program) else{return false;};
                let id=ids.remove(from); ids.insert(to,id);
                ids.clone()
            };
            if let Err(e)=save_pins(&ids_new){eprintln!("Melania: cannot save pins: {e}");}
            // Update the GTK widget order to match the new model order.
            let mut widgets=Vec::new();let mut current=area.first_child();
            while let Some(child)=current {current=child.next_sibling();widgets.push(child);}
            let mut previous:Option<gtk::Widget>=None;
            for id in ids_new {
                if let Some(widget)=widgets.iter().find(|w|w.widget_name().as_str()==id) {
                    area.reorder_child_after(widget,previous.as_ref());
                    previous=Some(widget.clone());
                }
            }
            destination.grab_focus();true
        });
        b.add_controller(drop);
        b.set_widget_name(program);
        pinned_buttons.borrow_mut().push((program.to_string(), b.clone()));
        pinned.append(&b);
    }
    bar.append(&pinned);
    // XFCE-like horizontal overflow rather than squeezing or hiding windows.
    // The original running-window buttons remain untouched inside the viewport.
    let running = running_window_strip(pinned_buttons);
    let running_scroll = gtk::ScrolledWindow::new();
    running_scroll.add_css_class("melania-running-scroll");
    running_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    running_scroll.set_min_content_width(140);
    running_scroll.set_max_content_width(610);
    running_scroll.set_hexpand(true);
    running_scroll.set_child(Some(&running));
    bar.append(&running_scroll);
    bar.append(&workspace_strip());
    let space=GtkBox::new(Orientation::Horizontal,1);space.set_hexpand(true);bar.append(&space);
    // Shell-only 20 px symbolic glyphs: these names are supplied by Melania Aero
    // Classic and remain overridable by system icon-theme fallback.
    let status=Button::new();status.add_css_class("melania-system");
    status.add_css_class("melania-tray-button");
    status.set_tooltip_text(Some("Rete, Bluetooth, audio e alimentazione"));
    let status_row=GtkBox::new(Orientation::Horizontal,8);
    for icon_name in ["network-wireless-signal-excellent-symbolic",
        "bluetooth-active-symbolic", "audio-volume-high-symbolic", "battery-good-symbolic"] {
        let icon=Image::from_icon_name(icon_name);
        icon.set_pixel_size(19);
        status_row.append(&icon);
    }
    status.set_child(Some(&status_row));
    let qs=quick.clone();let sm=start.clone();let nt=notifications.clone();
    status.connect_clicked(move |_|{nt.set_visible(false);toggle_surface(&qs,&sm);});bar.append(&status);
    let notification_button=Button::new();
    notification_button.set_tooltip_text(Some("Centro notifiche"));
    notification_button.add_css_class("melania-tray-button");
    let notification_row=GtkBox::new(Orientation::Horizontal,4);
    let notice_icon=Image::from_icon_name("preferences-system-notifications-symbolic");
    notice_icon.set_pixel_size(19);
    notification_row.append(&notice_icon);
    let badge=Label::new(None);badge.add_css_class("melania-notice-count");
    notification_row.append(&badge);
    notification_button.set_child(Some(&notification_row));
    let mut previous_unread=u32::MAX;
    glib::timeout_add_local(Duration::from_millis(800),move || {
        if let Ok(path)=melania_notify_core::snapshot_path() {
            if let Ok(snapshot)=melania_notify_core::read_snapshot(&path) {
                let unread=snapshot.notifications.iter().filter(|n|n.unread).count() as u32;
                if unread!=previous_unread {
                    if unread == 0 { badge.set_label(""); }
                    else { badge.set_label(&unread.to_string()); }
                    previous_unread=unread;
                }
            }
        }
        glib::ControlFlow::Continue
    });
    notification_button.add_css_class("melania-system");
    let notify=notifications.clone();let sm=start.clone();let qs=quick.clone();
    notification_button.connect_clicked(move |_|{
        sm.set_visible(false);qs.set_visible(false);
        if notify.is_visible(){notify.set_visible(false);}else{notify.present();}
    });
    bar.append(&notification_button);
    let clock=Button::with_label("--:--");clock.add_css_class("melania-system");
    clock.add_css_class("melania-clock");
    let refresh=clock.clone();
    let update=move || {let time=glib::DateTime::now_local().ok().and_then(|t|t.format("%H:%M\n%d/%m/%Y").ok());
        refresh.set_label(time.as_deref().unwrap_or("--:--"));};
    update();glib::timeout_add_seconds_local(20,move ||{update();glib::ControlFlow::Continue});
    let cal=calendar.clone();let qs=quick.clone();let sm=start.clone();let nt=notifications.clone();
    clock.connect_clicked(move |_|{
        qs.set_visible(false);sm.set_visible(false);nt.set_visible(false);
        if cal.is_visible(){cal.set_visible(false);}else{cal.present();}
    });bar.append(&clock);
    w.set_child(Some(&bar));w
}
fn build(app:&Application){
    setup_melania_icon_theme();
    if !gtk4_layer_shell::is_supported() {
        eprintln!("Melania Panel requires Wayland with zwlr_layer_shell_v1 (try the nested compositor)");
        return;
    }
    let preferences = melania_config::load().unwrap_or_default();
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_property("gtk-enable-animations",
            preferences.animations && !preferences.reduce_motion);
    }
    let css=gtk::CssProvider::new();css.load_from_data(include_str!("../assets/panel.css"));
    if let Some(display)=gdk::Display::default(){gtk::style_context_add_provider_for_display(&display,&css,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);}
    let start=start_window(app);
    let notifications=notifications_window(app);
    let quick=quick_window(app,&notifications);
    let calendar=calendar_window(app);
    // Do not present popup windows until the user clicks.
    let bar=taskbar(app,&start,&quick,&notifications,&calendar);bar.present();
}

/// Prefer Melania's freedesktop icon theme inside Melania applications.
/// During development, assets can be loaded directly from the source tree;
/// packaged installations use XDG_DATA_HOME/icons/Melania instead.
/// Does not change the icon theme of the user's other desktop sessions.
fn setup_melania_icon_theme() {
    let prefs=melania_config::load().unwrap_or_default();
    if let Some(display)=gtk::gdk::Display::default() {
        let icons=gtk::IconTheme::for_display(&display);
        for location in melania_config::bundled_icon_paths() {
            if location.is_dir() {icons.add_search_path(&location);}
        }
    }
    if let Some(settings)=gtk::Settings::default() {
        settings.set_property("gtk-icon-theme-name",
            melania_config::resolved_icon_theme(&prefs));
    }
}

fn main()->glib::ExitCode {
    let app=Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);app.run()
}
