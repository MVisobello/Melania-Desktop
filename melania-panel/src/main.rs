//! First independent GTK4 layer-shell taskbar for Melania.
//! This requires a compositor advertising zwlr_layer_shell_v1.
//! Window list comes from compositor IPC; thumbnails still require compositor capture.
mod window_groups;
mod session_actions;
mod start_history;
mod panel_preferences;
use gtk::{self, gdk, gio, glib, prelude::*};
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Entry, Image, Label, Orientation, Picture};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use melania_services as services;
use std::{cell::{Cell, RefCell}, collections::VecDeque, fs, io::Write, os::unix::{fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt}, net::UnixDatagram}, path::{Path, PathBuf}, process::Command, rc::Rc, thread, time::{Duration, Instant}};
use melania_control::{request as control_request, current_workspace, start_toggle_socket_path, Command as WindowCommand, WindowInfo, SOCKET_ENV};

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
    let appearance = melania_config::load().unwrap_or_default();
    w.add_css_class(&format!("melania-theme-{}", appearance.appearance_theme()));
    // gtk4-layer-shell must be initialized before any realization.
    w.init_layer_shell();w.set_namespace(Some(namespace));w.set_layer(kind);
    for &edge in anchors {w.set_anchor(edge,true);}
    if anchors.contains(&Edge::Bottom) {w.set_margin(Edge::Bottom,margin);}
    if anchors.contains(&Edge::Top) {w.set_margin(Edge::Top,margin);}
    if anchors.contains(&Edge::Left) {w.set_margin(Edge::Left,margin);}
    if anchors.contains(&Edge::Right) {w.set_margin(Edge::Right,margin);}
    w.set_exclusive_zone(exclusive);
    w
}
/// Smithay 0.7 retains a pre-commit layer role validator on the underlying
/// wl_surface after GTK destroys a layer role; GTK then commits an empty
/// buffer and the validator disconnects the whole Panel. Preserve the real
/// layer role instead: slide Start completely outside the output and release
/// keyboard focus, then bring it back on demand. The hidden surface is still
/// alive, so a second Super press does not recreate a broken Wayland role.
const START_HIDE_OFFSET: i32 = -4000;
const START_LEFT_MARGIN: i32 = 8;
fn start_search_entry(w:&ApplicationWindow) -> Option<Entry> {
    let root=w.child()?;
    let footer=root.last_child()?;
    footer.first_child()?.downcast::<Entry>().ok()
}
fn hide_start_surface(w:&ApplicationWindow) {
    if !w.is_visible() || w.has_css_class("melania-start-concealed") {return;}
    w.add_css_class("melania-start-concealed");
    w.set_keyboard_mode(KeyboardMode::None);
    w.set_margin(Edge::Left,START_HIDE_OFFSET);
    w.set_opacity(0.0);
    if let Some(entry)=start_search_entry(w) { entry.set_text(""); }
}
fn present_start_surface(w:&ApplicationWindow) {
    let was_hidden=w.has_css_class("melania-start-concealed");
    w.remove_css_class("melania-start-concealed");
    w.set_margin(Edge::Left,START_LEFT_MARGIN);
    w.set_opacity(1.0);
    w.set_keyboard_mode(KeyboardMode::Exclusive);
    if !w.is_visible() {w.present();}
    if was_hidden {
        if let Some(entry)=start_search_entry(w) {
            glib::idle_add_local_once(move || { let _=entry.grab_focus(); });
        }
    }
}
// Smithay retains a layer-role validator across GTK surface unmaps. Hiding
// any auxiliary layer with set_visible(false) after it has been presented can
// disconnect the *entire panel client*, including the taskbar. Keep the role
// mapped, move the popup entirely beyond the output, and suppress its focus.
const POPUP_HIDE_RIGHT_MARGIN: i32 = -4000;
const POPUP_RIGHT_MARGIN: i32 = 12;
fn popup_is_open(w:&ApplicationWindow) -> bool {
    w.is_visible() && !w.has_css_class("melania-popup-concealed")
}
fn hide_popup_surface(w:&ApplicationWindow) {
    if !popup_is_open(w) { return; }
    w.add_css_class("melania-popup-concealed");
    w.set_keyboard_mode(KeyboardMode::None);
    w.set_margin(Edge::Right, POPUP_HIDE_RIGHT_MARGIN);
    w.set_opacity(0.0);
}
fn present_popup_surface(w:&ApplicationWindow) {
    w.remove_css_class("melania-popup-concealed");
    w.set_margin(Edge::Right, POPUP_RIGHT_MARGIN);
    w.set_opacity(1.0);
    w.set_keyboard_mode(KeyboardMode::OnDemand);
    if !w.is_visible() { w.present(); }
}
fn toggle_surface(w:&ApplicationWindow,other:&ApplicationWindow) {
    if other.has_css_class("melania-start-surface") {
        hide_start_surface(other);
    } else { hide_popup_surface(other); }
    if w.has_css_class("melania-start-surface") {
        if w.is_visible() && !w.has_css_class("melania-start-concealed") {
            hide_start_surface(w);
        } else {present_start_surface(w);}
    } else if popup_is_open(w) {
        hide_popup_surface(w);
    } else {
        present_popup_surface(w);
    }
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
        "MUSIC" => &["Musica", "Music"],
        "VIDEOS" => &["Video", "Videos"],
        _ => &[],
    };
    for name in candidates {
        let path = home.join(name);
        if path.is_dir() { return path; }
    }
    home
}

// Prefer Melania artwork, but never leave a blank GTK image when a theme
// variant or a portable preview does not ship the requested icon name.
fn icon_with_fallback(primary: &str, fallback: &str) -> Image {
    let available = gdk::Display::default()
        .map(|display| gtk::IconTheme::for_display(&display).has_icon(primary))
        .unwrap_or(false);
    Image::from_icon_name(if available { primary } else { fallback })
}

// Window app IDs may be reverse-DNS names or executable names rather than
// freedesktop icon names. Normalize our own apps and resolve installed desktop
// launchers before falling back to a generic, visible taskbar glyph.
fn taskbar_app_icon(app_id: &str) -> Image {
    let lower=app_id.to_ascii_lowercase();
    if lower.contains("melania") && lower.contains("files") || lower=="nautilus" || lower=="nemo" {
        return icon_with_fallback("system-file-manager","folder");
    }
    if lower.contains("melania") && lower.contains("settings") {
        return icon_with_fallback("preferences-system","applications-system");
    }
    // A full GIO desktop-app enumeration is expensive. Taskbar refreshes
    // must not repeat it for each window, especially during session startup.
    // Cache successful icon lookups on the GTK main thread for this session.
    thread_local! {
        static ICON_CACHE: RefCell<std::collections::HashMap<String, Option<gio::Icon>>> =
            RefCell::new(std::collections::HashMap::new());
    }
    let cached=ICON_CACHE.with(|cache| {
        if let Some(icon)=cache.borrow().get(app_id) {return icon.clone();}
        let desktop_id=format!("{app_id}.desktop");
        let icon=gio::AppInfo::all().into_iter().find(|app| {
            app.id().as_deref()==Some(app_id)
                || app.id().as_deref()==Some(desktop_id.as_str())
        }).and_then(|app|app.icon());
        // Do not cache misses: an application installed during this session
        // can still gain its correct icon on the next window-list update.
        if icon.is_some() {
            cache.borrow_mut().insert(app_id.to_owned(),icon.clone());
        }
        icon
    });
    if let Some(icon)=cached {return Image::from_gicon(&icon);}
    icon_with_fallback(app_id,"application-x-executable")
}

// Draw the two core Start icons independently of incomplete installed themes.
// The Files pictogram is the conventional folder used by Linux file managers.
fn start_core_icon(kind: &'static str) -> gtk::DrawingArea {
    let area=gtk::DrawingArea::new();
    area.set_content_width(27);
    area.set_content_height(27);
    area.set_draw_func(move |_,cr,w,h| {
        if w<=0 || h<=0 {return;}
        cr.scale(w as f64/28.0,h as f64/28.0);
        if kind=="files" {
            // Folder tab and body, with a subtle Fluent-style highlight.
            cr.set_source_rgb(0.19,0.52,0.82);
            cr.rectangle(2.5,6.0,10.5,5.5);
            let _=cr.fill();
            cr.set_source_rgb(0.25,0.65,0.95);
            cr.rectangle(2.5,9.0,23.0,15.5);
            let _=cr.fill();
            cr.set_source_rgb(0.49,0.79,1.0);
            cr.rectangle(4.0,10.5,20.0,2.2);
            let _=cr.fill();
        } else {
            // Recognisable settings gear, not a generic blue application tile.
            cr.set_source_rgb(0.39,0.51,0.66);
            for tooth in 0..8 {
                let angle=(tooth as f64)*std::f64::consts::TAU/8.0;
                cr.save().ok();
                cr.translate(14.0,14.0);
                cr.rotate(angle);
                cr.rectangle(-2.4,-12.0,4.8,6.0);
                let _=cr.fill();
                cr.restore().ok();
            }
            cr.arc(14.0,14.0,9.0,0.0,std::f64::consts::TAU);
            let _=cr.fill();
            cr.set_source_rgb(0.95,0.97,1.0);
            cr.arc(14.0,14.0,3.9,0.0,std::f64::consts::TAU);
            let _=cr.fill();
        }
    });
    area
}

fn icon_row(text: &str, icon_name: &str, class: &str) -> Button {
    let button = Button::new();
    button.add_css_class(class);
    let row = GtkBox::new(Orientation::Horizontal, 10);
    let resolved = match icon_name {
        // These standard freedesktop names are shipped by Adwaita, Nemo and
        // Nautilus icon themes, even when Melania app-ID aliases are absent.
        "org.melaniadesktop.Settings" => "preferences-system",
        "org.melaniadesktop.Files" => "system-file-manager",
        other => other,
    };
    if icon_name == "org.melaniadesktop.Files" || icon_name == "org.melaniadesktop.Settings" {
        let icon=start_core_icon(if icon_name == "org.melaniadesktop.Files" {"files"} else {"settings"});
        icon.add_css_class("melania-menu-app-icon");
        row.append(&icon);
    } else {
        let icon=icon_with_fallback(resolved,"application-x-executable");
        icon.set_pixel_size(if class == "melania-shortcut" {18} else {27});
        icon.add_css_class("melania-menu-app-icon");
        row.append(&icon);
    }
    let caption = label(text, "melania-row-text");
    caption.set_hexpand(true);
    row.append(&caption);
    button.set_child(Some(&row));
    button
}


/// Win7-like Start sidebar links use quiet symbolic glyphs instead of colorful
/// folder tiles. The system icon theme supplies the *-symbolic artwork.
fn classic_link_row(text: &str, icon_name: &str) -> Button {
    let button=Button::new();
    button.add_css_class("melania-classic-link");
    let row=GtkBox::new(Orientation::Horizontal,9);
    let image=Image::from_icon_name(icon_name);
    image.set_pixel_size(16);
    image.add_css_class("melania-classic-symbolic");
    row.append(&image);
    let caption=label(text,"melania-classic-link-text");
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    caption.set_max_width_chars(25);
    caption.set_hexpand(true);
    row.append(&caption);
    button.set_child(Some(&row));
    button
}


/// Original vector glyphs drawn in Cairo, not themed freedesktop aliases.
/// Several user icon packs remap system-shutdown or avatar-default to a blank
/// dot/document: drawing these two controls avoids that and needs no assets
/// installed separately from the Melania Panel binary.
fn classic_control_glyph(kind: &'static str, light: bool) -> gtk::DrawingArea {
    let drawing=gtk::DrawingArea::new();
    let size=if kind=="account" {27} else {22};
    drawing.set_content_width(size);
    drawing.set_content_height(size);
    drawing.set_draw_func(move |_, cr, width, height| {
        if width<=0 || height<=0 { return; }
        cr.scale(width as f64 / 24.0, height as f64 / 24.0);
        if light { cr.set_source_rgb(0.20,0.30,0.42); }
        else { cr.set_source_rgb(0.94,0.97,1.0); }
        if kind=="account" {
            cr.arc(12.0,7.4,4.0,0.0,std::f64::consts::TAU);
            let _=cr.fill();
            cr.move_to(3.9,21.0);
            cr.curve_to(4.3,15.0,8.0,13.6,12.0,13.6);
            cr.curve_to(16.0,13.6,19.7,15.0,20.1,21.0);
            cr.close_path();
            let _=cr.fill();
        } else {
            // Familiar power glyph with a deliberate break at the top.
            cr.set_line_width(2.5);
            cr.arc(12.0,12.5,8.0,-1.03,4.17);
            let _=cr.stroke();
            cr.move_to(12.0,2.0);
            cr.line_to(12.0,11.2);
            let _=cr.stroke();
        }
    });
    drawing
}

fn confirm_session_action(parent: &ApplicationWindow, action: session_actions::Action) {
    // A separate positive response is required for every action, including
    // log out; merely opening Start can never change system/session state.
    if let Err(reason) = session_actions::availability(action) {
        eprintln!("Melania session action unavailable: {reason}");
        return;
    }
    let dialog = gtk::MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(gtk::MessageType::Question)
        .buttons(gtk::ButtonsType::None)
        .text(format!("{}?", action.label()))
        .secondary_text("Confermi questa operazione? I documenti non salvati potrebbero andare persi.")
        .build();
    dialog.add_button("Annulla", gtk::ResponseType::Cancel);
    dialog.add_button("Conferma", gtk::ResponseType::Accept);
    let owner = parent.clone();
    dialog.connect_response(move |dialog, response| {
        dialog.close();
        if response != gtk::ResponseType::Accept { return; }
        if let Err(reason) = session_actions::perform(action) {
            let error = gtk::MessageDialog::builder()
                .transient_for(&owner)
                .modal(true)
                .message_type(gtk::MessageType::Error)
                .buttons(gtk::ButtonsType::Close)
                .text("Operazione non riuscita")
                .secondary_text(&reason)
                .build();
            error.connect_response(|dialog, _| dialog.close());
            error.present();
        }
    });
    dialog.present();
}

fn start_window(app:&Application)->ApplicationWindow {
    // Two visually distinct Win7-style columns, with the original Melania
    // branding and native GTK actions. The taskbar reserves 52 logical px;
    // its GTK CSS adds ~6 px of visible height. Matching that offset removes
    // the empty strip seen between Start and the taskbar in nested Wayland.
    let w=make_surface(app,Layer::Overlay,&[Edge::Left,Edge::Bottom],510,565,0,"melania-start",0);
    w.set_margin(Edge::Left,8);
    w.set_margin(Edge::Bottom,6);
    w.add_css_class("melania-start-surface");
    // Layer-shell keyboard focus is explicitly granted by the Melania compositor
    // when this overlay is mapped; search can receive keystrokes immediately.
    w.set_keyboard_mode(KeyboardMode::Exclusive);
    let root=GtkBox::new(Orientation::Vertical,0);
    root.add_css_class("melania-panel");
    root.add_css_class("melania-start-panel");
    // Preserve the approved Start surface without an added Melania heading.
    // Keep search instantly focused, but place its widget in the footer like Windows 7.
    if melania_config::load().unwrap_or_default().high_contrast {
        root.add_css_class("high-contrast");
    }
    let query=Entry::builder().placeholder_text("Cerca app, file o impostazioni…").build();
    query.add_css_class("melania-entry");
    // Start always opens ready to type; closing clears the previous filter.
    let search_focus=query.clone();
    w.connect_notify_local(Some("visible"), move |surface,_| {
        if surface.is_visible() {
            // GTK may not have mapped the layer when "visible" changes.
            // Defer the focus grab so typed text reaches the search entry.
            let query=search_focus.clone();
            glib::idle_add_local_once(move || { let _=query.grab_focus(); });
        } else { search_focus.set_text(""); }
    });
    let mapped_search=query.clone();
    w.connect_map(move |_| { let _=mapped_search.grab_focus(); });
    // Escape dismisses Start without modifying the current desktop session.
    let escape=gtk::EventControllerKey::new();
    let escape_owner=w.clone();
    escape.connect_key_pressed(move |_,key,_,_| {
        if key==gdk::Key::Escape {
            hide_start_surface(&escape_owner);
            glib::Propagation::Stop
        } else { glib::Propagation::Proceed }
    });
    w.add_controller(escape);
    let columns=GtkBox::new(Orientation::Horizontal,0);
    columns.add_css_class("melania-start-columns");
    let left=GtkBox::new(Orientation::Vertical,8);
    left.set_hexpand(true);
    left.add_css_class("melania-start-program-column");
    let scroll=gtk::ScrolledWindow::new();
    // Do not set vexpand: it overrides the content-height cap and stretches
    // the shell layer all the way to the top edge on smaller displays.
    scroll.set_vexpand(false);
    scroll.set_hexpand(true);
    scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
    scroll.set_propagate_natural_height(true);
    scroll.set_min_content_height(220);
    scroll.set_max_content_height(302);
    let apps_box=GtkBox::new(Orientation::Vertical,1);
    apps_box.add_css_class("melania-start-apps");
    // Search-only results remain hidden in the normal classic app list.
    let searchable:Rc<RefCell<Vec<(Button,String,bool)>>>=Rc::new(RefCell::new(Vec::new()));
    for (name,program,icon) in [
        ("Melania Files", "melania-files", "org.melaniadesktop.Files"),
        ("Impostazioni", "melania-settings", "org.melaniadesktop.Settings"),
    ] {
        let button=icon_row(name,icon,"melania-action");
        let menu=w.clone();
        button.connect_clicked(move |_| {
            match Command::new(program).spawn() {
                Ok(_) => {
                    let id=if program=="melania-files" {
                        "org.melaniadesktop.Files"
                    } else { "org.melaniadesktop.Settings" };
                    start_history::record_launch(id);
                    hide_start_surface(&menu);
                }
                Err(error) => eprintln!("Melania app launch: {error}"),
            }
        });
        attach_pin_context(&button,program.to_string());
        apps_box.append(&button);
        searchable.borrow_mut().push((button,name.to_lowercase(),false));
    }
    // Keep the installed-app list separate so it can be filled after
    // the first Start presentation rather than delaying the taskbar.
    let installed_box=GtkBox::new(Orientation::Vertical,1);
    apps_box.append(&installed_box);
    // Share a single verified Gio catalog between the installed-app list
    // and the Recent section. A second AppInfo::all() on Start opening
    // previously duplicated the slowest synchronous operation on the UI.
    let installed_cache:Rc<RefCell<Vec<gio::AppInfo>>>=Rc::new(RefCell::new(Vec::new()));
    let refresh_recent:Rc<RefCell<Option<Rc<dyn Fn()>>>>=Rc::new(RefCell::new(None));
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
            } else { hide_start_surface(&menu); }
        });
        searchable.borrow_mut().push((button.clone(),format!("{} {}",caption.to_lowercase(),keywords),true));
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
            } else {hide_start_surface(&menu);}
        });
        searchable.borrow_mut().push((button.clone(),format!("{} {}",caption.to_lowercase(),kind.to_lowercase()),true));
        apps_box.append(&button);
    }
    let empty=label("Nessun risultato: applicazioni, cartelle o impostazioni", "melania-sub");
    empty.set_visible(false);apps_box.append(&empty);
    let searchable_for_filter=searchable.clone();
    query.connect_changed(move |q| {
        let term=q.text().trim().to_lowercase();
        if !term.is_empty() &&
            std::env::var_os("MELANIA_SMOKE_OPEN_START").is_some() {
            // Record only the fact that typing reached search in isolated CI,
            // not the user's actual input content.
            eprintln!("MELANIA_START_SEARCH_RECEIVED");
        }
        let mut visible=0;
        for (button,name,search_only) in searchable_for_filter.borrow().iter() {
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
        // Release the RefCell borrow before triggering a click: launching
        // an app conceals Start and clears the search entry synchronously.
        let first=searchable_for_enter.borrow().iter()
            .find(|(_,name,_)|name.contains(&term))
            .map(|(button,_,_)|button.clone());
        if let Some(button)=first {button.emit_clicked();}
    });
    // Build app widgets only after Start becomes visible. Batch the work
    // to avoid a one-shot GTK stall when many .desktop launchers are installed.
    let installed_once=Rc::new(Cell::new(false));
    let apps_for_open=installed_box.clone();
    let all_searchable=searchable.clone();
    let menu_for_apps=w.clone();
    let query_for_apps=query.clone();
    let cache_for_apps=installed_cache.clone();
    let recents_after_catalog=refresh_recent.clone();
    w.connect_notify_local(Some("visible"),move |surface,_|{
        if !surface.is_visible() || installed_once.replace(true){return;}
        let installed_box=apps_for_open.clone();
        let searchable=all_searchable.clone();
        let menu=menu_for_apps.clone();
        let query=query_for_apps.clone();
        let cache_for_apps=cache_for_apps.clone();
        let recents_after_catalog=recents_after_catalog.clone();
        glib::timeout_add_local_once(Duration::from_millis(75),move ||{
            let scan_started=Instant::now();
            let mut apps=gio::AppInfo::all().into_iter().filter(|app|{
                if !app.should_show(){return false;}
                let identifier=app.id().map(|id|id.to_string()).unwrap_or_default();
                let normalized=identifier.trim_end_matches(".desktop");
                normalized!="org.melaniadesktop.Files" && normalized!="org.melaniadesktop.Settings"
            }).collect::<Vec<_>>();
            apps.sort_by_key(|app|app.name().to_string().to_lowercase());
            if std::env::var_os("MELANIA_PROFILE_START").is_some() {
                eprintln!("MELANIA_START_CATALOG_MS={} installed={}",
                    scan_started.elapsed().as_millis(),apps.len());
            }
            let widget_started=Instant::now();
            let widget_count=apps.len();
            *cache_for_apps.borrow_mut()=apps.clone();
            // The recent list now resolves IDs against the same snapshot.
            if let Some(refresh)=recents_after_catalog.borrow().as_ref(){
                refresh();
            }
            let pending=Rc::new(RefCell::new(VecDeque::from(apps)));
            glib::timeout_add_local(Duration::from_millis(16),move ||{
                for _ in 0..12 {
                    let Some(app_info)=pending.borrow_mut().pop_front() else {
                        return glib::ControlFlow::Break;
                    };
                    let name=app_info.name().to_string();
                    let name_lower=name.to_lowercase();
                    let button=Button::new();button.add_css_class("melania-action");
                    let row=GtkBox::new(Orientation::Horizontal,12);
                    let icon=app_info.icon().map(|i|Image::from_gicon(&i))
                        .unwrap_or_else(||Image::from_icon_name("application-x-executable"));
                    icon.set_pixel_size(27);
                    icon.add_css_class("melania-menu-app-icon");
                    row.append(&icon);
                    let caption=label(&name,"melania-row-text");
                    caption.set_hexpand(true);
                    row.append(&caption);
                    button.set_child(Some(&row));
                    let window=menu.clone();
                    let history_id=app_info.id().map(|id|id.to_string());
                    let pin_id=history_id.clone();
                    button.connect_clicked(move |_|{
                        match app_info.launch(&[],None::<&gio::AppLaunchContext>){
                            Ok(())=>{
                                if let Some(id)=history_id.as_deref(){
                                    start_history::record_launch(id);
                                }
                                hide_start_surface(&window);
                            },
                            Err(error)=>eprintln!("Melania app launch: {error}"),
                        }
                    });
                    if let Some(id)=pin_id{
                        // Trusted ID from this Gio installed-app enumeration.
                        attach_pin_context_trusted(&button,id);
                    }
                    let term=query.text().trim().to_lowercase();
                    button.set_visible(term.is_empty() || name_lower.contains(&term));
                    installed_box.append(&button);
                    searchable.borrow_mut().push((button,name_lower,false));
                }
                if pending.borrow().is_empty(){
                    if std::env::var_os("MELANIA_PROFILE_START").is_some() {
                        eprintln!("MELANIA_START_ROWS_MS={} count={}",
                            widget_started.elapsed().as_millis(),widget_count);
                    }
                    glib::ControlFlow::Break
                }else{glib::ControlFlow::Continue}
            });
        });
    });
    scroll.set_child(Some(&apps_box));
    // A bounded recent list, refreshed whenever Start opens. Store only
    // freedesktop desktop IDs, never command lines or filenames. Unknown or
    // uninstalled entries are skipped without launching anything.
    let recent_heading=label("RECENTI","melania-section-label");
    recent_heading.set_visible(false);
    left.append(&recent_heading);
    let recent_list=GtkBox::new(Orientation::Vertical,1);
    recent_list.add_css_class("melania-recent-list");
    recent_list.set_visible(false);
    left.append(&recent_list);
    let menu_for_recent=w.clone();
    let installed_for_recent=installed_cache.clone();
    let refresh_recents:Rc<dyn Fn()>=Rc::new(move || {
        while let Some(child)=recent_list.first_child(){recent_list.remove(&child);}
        let installed=installed_for_recent.borrow();
        let mut count=0;
        for id in start_history::recent() {
            if count>=4 {break;}
            if id=="org.melaniadesktop.Files" || id=="org.melaniadesktop.Settings" {
                let (name,program,icon)=if id=="org.melaniadesktop.Files" {
                    ("Melania Files","melania-files","org.melaniadesktop.Files")
                } else {("Impostazioni","melania-settings","org.melaniadesktop.Settings")};
                let launch=icon_row(name,icon,"melania-action");
                let menu=menu_for_recent.clone();
                launch.connect_clicked(move |_| {
                    match Command::new(program).spawn() {
                        Ok(_)=>{start_history::record_launch(if program=="melania-files" {
                            "org.melaniadesktop.Files"
                        } else {"org.melaniadesktop.Settings"});hide_start_surface(&menu);},
                        Err(error)=>eprintln!("Melania recent app launch: {error}"),
                    }
                });
                recent_list.append(&launch);
                count+=1;
                continue;
            }
            let Some(info)=installed.iter().find(|app| app.should_show() &&
                app.id().as_deref()==Some(id.as_str())).cloned() else {continue;};
            let launch=Button::new();
            launch.add_css_class("melania-action");
            let row=GtkBox::new(Orientation::Horizontal,8);
            let image=info.icon().map(|icon|Image::from_gicon(&icon))
                .unwrap_or_else(||Image::from_icon_name("application-x-executable"));
            image.set_pixel_size(27);
            row.append(&image);
            let caption=label(&info.name(),"melania-row-text");
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            caption.set_max_width_chars(13);
            row.append(&caption);
            launch.set_child(Some(&row));
            let menu=menu_for_recent.clone();
            launch.connect_clicked(move |_| {
                match info.launch(&[],None::<&gio::AppLaunchContext>) {
                    Ok(())=>{start_history::record_launch(&id);hide_start_surface(&menu);},
                    Err(error)=>eprintln!("Melania recent app launch: {error}"),
                }
            });
            recent_list.append(&launch);
            count+=1;
        }
        recent_heading.set_visible(count>0);
        recent_list.set_visible(count>0);
    });
    *refresh_recent.borrow_mut()=Some(refresh_recents.clone());
    // The GTK layer-shell surface remains mapped while concealed. Opacity
    // changes, not the visible property, mark subsequent Start openings.
    let recent_visible=refresh_recents.clone();
    w.connect_notify_local(Some("visible"),move |surface,_|{
        if surface.is_visible(){recent_visible();}
    });
    let recent_reopened=refresh_recents.clone();
    w.connect_notify_local(Some("opacity"),move |surface,_|{
        if surface.is_visible() && surface.opacity()>0.5 {recent_reopened();}
    });
    // Recent launches belong with applications on the left, not among the
    // stable system locations on the right.
    left.append(&label("TUTTE LE APPLICAZIONI","melania-section-label"));
    left.append(&scroll);
    columns.append(&left);
    let right=GtkBox::new(Orientation::Vertical,1);
    right.set_valign(gtk::Align::Fill);
    right.add_css_class("melania-start-shortcuts");
    right.set_size_request(158,-1);
    // Windows 7 places the current account above the system shortcuts.
    let username=std::env::var("USER").ok()
        .filter(|name|!name.trim().is_empty())
        .unwrap_or_else(||"Utente".to_owned());
    let profile=GtkBox::new(Orientation::Horizontal,9);
    profile.add_css_class("melania-start-user");
    // A single larger, circular account avatar lives beside the user name at
    // the TOP of the system column, exactly like the classic Start portrait.
    let account=Button::new();
    account.add_css_class("melania-account-orb");
    account.set_tooltip_text(Some("Account · Impostazioni utente"));
    let light_controls=melania_config::load().unwrap_or_default().appearance_theme()=="light";
    let account_icon=classic_control_glyph("account",light_controls);
    account.set_child(Some(&account_icon));
    let account_menu=w.clone();
    account.connect_clicked(move |_| {
        match Command::new("melania-settings").arg("account").spawn() {
            Ok(_)=>hide_start_surface(&account_menu),
            Err(error)=>eprintln!("Melania account settings: {error}"),
        }
    });
    profile.append(&account);
    let username_label=label(&username,"melania-start-user-name");
    username_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    username_label.set_max_width_chars(14);
    profile.append(&username_label);
    right.append(&profile);
    // The set of destinations mirrors classic Windows 7, but it opens real
    // Linux folders and Melania Settings sections. No Windows-only commands.
    for (name,icon_name,action,arg) in [
        ("Documenti","folder-documents-symbolic","folder","DOCUMENTS"),
        ("Immagini","folder-pictures-symbolic","folder","PICTURES"),
        ("Musica","folder-music-symbolic","folder","MUSIC"),
        ("Video","folder-videos-symbolic","folder","VIDEOS"),
        ("Download","folder-download-symbolic","folder","DOWNLOAD"),
        ("Computer","computer-symbolic","computer",""),
        ("Pannello di controllo","preferences-system-symbolic","settings","system"),
        ("Dispositivi e stampanti","printer-symbolic","settings","devices"),
        ("Programmi predefiniti","preferences-desktop-default-applications-symbolic","settings","applications"),
        ("Guida e supporto","help-browser-symbolic","help",""),
    ] {
        let button=classic_link_row(name,icon_name);
        // Stretch the ten real destinations through the column's remaining
        // height instead of leaving unused dark space below them.
        button.set_vexpand(true);
        if name=="Dispositivi e stampanti" {
            button.set_tooltip_text(Some("Apre Dispositivi in Impostazioni; la gestione completa delle stampanti non è ancora disponibile."));
        }
        if name=="Programmi predefiniti" {
            button.set_tooltip_text(Some("Apre Applicazioni in Impostazioni; la scelta dei programmi predefiniti è in sviluppo."));
        }
        let menu=w.clone();
        button.connect_clicked(move |_| {
            let result=match action {
                "folder"=>Command::new("melania-files")
                    .arg(xdg_shortcut_directory(arg)).spawn(),
                "computer"=>Command::new("melania-files").arg("/").spawn(),
                "settings"=>Command::new("melania-settings").arg(arg).spawn(),
                "help"=>Command::new("xdg-open")
                    .arg("https://github.com/MVisobello/Melania-Desktop#readme").spawn(),
                _=>return,
            };
            match result {
                Ok(_)=>{hide_start_surface(&menu);},
                Err(e)=>eprintln!("Melania Start shortcut {name}: {e}"),
            }
        });
        right.append(&button);
        if name=="Download" || name=="Computer" || name=="Dispositivi e stampanti" {
            let divider=gtk::Separator::new(Orientation::Horizontal);
            divider.add_css_class("melania-classic-divider");
            right.append(&divider);
        }
    }
    columns.append(&right);
    // Keep the two-column content above the footer: expanding this viewport
    // lets the search and power buttons hug the taskbar even when the
    // applications column contains fewer items than the available height.
    let body_scroll = gtk::ScrolledWindow::new();
    body_scroll.add_css_class("melania-start-body-scroll");
    body_scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
    body_scroll.set_propagate_natural_height(true);
    body_scroll.set_vexpand(true);
    body_scroll.set_min_content_height(280);
    body_scroll.set_max_content_height(452);
    body_scroll.set_child(Some(&columns));
    root.append(&body_scroll);
    let footer=GtkBox::new(Orientation::Horizontal,9);
    footer.add_css_class("melania-start-footer");
    // Keep the search field under the application column and the power
    // controls on the right, in the same places as Windows 7.
    query.set_hexpand(false);
    query.set_valign(gtk::Align::Center);
    query.set_size_request(250,-1);
    footer.append(&query);
    let footer_spacer=GtkBox::new(Orientation::Horizontal,0);
    footer_spacer.set_hexpand(true);
    footer.append(&footer_spacer);
    // One power icon, one click: open session choices. Never power off the
    // host directly from the Start footer; each action retains its own
    // availability guard and explicit confirmation dialog.
    let more=gtk::MenuButton::new();
    more.set_valign(gtk::Align::Center);
    more.set_child(Some(&classic_control_glyph("power",light_controls)));
    more.add_css_class("melania-session-more");
    more.add_css_class("melania-power-icon-button");
    more.set_tooltip_text(Some("Alimentazione e sessione"));
    let popover=gtk::Popover::new();
    popover.add_css_class("melania-session-popup");
    let choices=GtkBox::new(Orientation::Vertical,4);
    choices.add_css_class("melania-session-choices");
    for action in session_actions::Action::ALL {
        let choice=Button::with_label(action.label());
        choice.add_css_class("melania-session-choice");
        if let Err(reason)=session_actions::availability(action) {
            choice.set_sensitive(false);
            choice.set_tooltip_text(Some(&reason));
        }
        let owner=w.clone();
        choice.connect_clicked(move |_| confirm_session_action(&owner,action));
        choices.append(&choice);
    }
    if let Err(reason)=session_actions::availability(session_actions::Action::PowerOff) {
        let explanation=label(&reason,"melania-session-hint");
        explanation.set_wrap(true);
        explanation.set_max_width_chars(35);
        choices.append(&explanation);
    }
    popover.set_child(Some(&choices));
    more.set_popover(Some(&popover));
    footer.append(&more);
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
    notifications_button.connect_clicked(move |_| {hide_popup_surface(&myself);present_popup_surface(&other);});
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
/// Metadata-only change detection keeps installed-app enumeration off the
/// periodic UI tick. Atomic XDG file replacement changes the inode.
fn file_stamp(path:&Path)->Option<(u64,u64,i64,i64)> {
    fs::metadata(path).ok().map(|m|(m.ino(),m.len(),m.mtime(),m.mtime_nsec()))
}
fn pins_path()->Option<PathBuf> {
    let dir=std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|PathBuf::from(h).join(".config")))?;
    Some(dir.join("melania").join("pinned-apps"))
}

/// Pin IDs must resolve to an installed desktop application or one of our
/// two internal launchers. Never interpret a saved value as a command.
fn installed_pin_app(id:&str)->Option<gio::AppInfo> {
    if id.len()>240 || id.chars().any(char::is_control){return None;}
    gio::AppInfo::all().into_iter().find(|app| {
        app.should_show() && app.id().as_deref()==Some(id)
    })
}
fn valid_pin(id:&str)->bool {
    matches!(id,"melania-files"|"melania-settings") || installed_pin_app(id).is_some()
}
fn load_pins()->Vec<String> {
    const DEFAULT:[&str;2]=["melania-files","melania-settings"];
    let Some(path)=pins_path() else{return DEFAULT.iter().map(|s|s.to_string()).collect();};
    if !path.exists(){return DEFAULT.iter().map(|s|s.to_string()).collect();}
    let Ok(text)=fs::read_to_string(path) else{return DEFAULT.iter().map(|s|s.to_string()).collect();};
    let mut ids=Vec::new();
    for id in text.lines().take(48) {
        if valid_pin(id) && !ids.iter().any(|x|x==id){ids.push(id.to_string());}
    }
    ids
}
fn save_pins(ids:&[String])->std::io::Result<()> {
    let Some(path)=pins_path() else {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound,"XDG config unavailable"));
    };
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    let temp=path.with_extension(format!("tmp-{}",std::process::id()));
    let mut file=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp)?;
    let write_result=(|| {
        for id in ids.iter().take(48) {
            if valid_pin(id){writeln!(file,"{id}")?;}
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temp,&path)
    })();
    if write_result.is_err(){let _=fs::remove_file(&temp);}
    write_result
}
fn toggle_pin(id:&str) {
    if !valid_pin(id){return;}
    let mut pins=load_pins();
    if let Some(n)=pins.iter().position(|s|s==id) {
        pins.remove(n);
    } else if pins.len()<48 {
        pins.push(id.to_string());
    }
    if let Err(error)=save_pins(&pins){eprintln!("Melania pin: {error}");}
}
/// A right-click popover in Start and on the taskbar for validated launchers.
fn attach_pin_context(widget:&Button,id:String) {
    if !valid_pin(&id){return;}
    attach_pin_context_trusted(widget,id);
}
/// Only callers with a builtin or Gio-discovered application may skip validation.
fn attach_pin_context_trusted(widget:&Button,id:String){
    let popup=gtk::Popover::new();
    popup.add_css_class("melania-pin-popover");
    popup.set_parent(widget);
    let action=Button::with_label("Aggiungi alla barra");
    action.add_css_class("melania-pin-choice");
    let options=GtkBox::new(Orientation::Vertical,3);
    options.add_css_class("melania-pin-menu");
    options.append(&action);
    popup.set_child(Some(&options));
    let action_id=id.clone();
    let close=popup.clone();
    action.connect_clicked(move |_|{toggle_pin(&action_id);close.popdown();});
    let click=gtk::GestureClick::new();
    click.set_button(3);
    click.connect_pressed(move |_,_,_,_| {
        action.set_label(if load_pins().iter().any(|s|s==&id) {
            "Rimuovi dalla barra"
        } else {"Aggiungi alla barra"});
        popup.popup();
    });
    widget.add_controller(click);
}
fn app_button(id:&str)->Option<Button> {
    let info=if matches!(id,"melania-files"|"melania-settings"){None}
        else {Some(installed_pin_app(id)?)};
    let b=Button::new();
    b.add_css_class("melania-pin");
    let row=GtkBox::new(Orientation::Horizontal,6);
    let (name,icon)=match id {
        "melania-files"=>("File".to_owned(),icon_with_fallback("system-file-manager","folder")),
        "melania-settings"=>("Impostazioni".to_owned(),icon_with_fallback("preferences-system","applications-system")),
        _=>{
            let app=info.as_ref()?;
            (app.name().to_string(),app.icon().map(|icon|Image::from_gicon(&icon))
                .unwrap_or_else(||Image::from_icon_name("application-x-executable")))
        }
    };
    icon.set_pixel_size(28);
    row.append(&icon);
    b.set_child(Some(&row));
    b.add_css_class("melania-icon-only");
    b.set_tooltip_text(Some(&name));
    let launch_id=id.to_string();
    b.connect_clicked(move |_|{
        match launch_id.as_str() {
            "melania-files"=>run("melania-files"),
            "melania-settings"=>run("melania-settings"),
            _=>if let Some(app)=installed_pin_app(&launch_id) {
                if let Err(error)=app.launch(&[],None::<&gio::AppLaunchContext>) {
                    eprintln!("Melania pinned app: {error}");
                }
            }
        }
    });
    attach_pin_context(&b,id.to_string());
    Some(b)
}
fn populate_pins(area:&GtkBox,buttons:&Rc<RefCell<Vec<(String,Button)>>>,
                 order:&Rc<RefCell<Vec<String>>>) {
    let ids=load_pins();
    *order.borrow_mut()=ids.clone();
    buttons.borrow_mut().clear();
    while let Some(widget)=area.first_child(){area.remove(&widget);}
    for id in ids {
        let Some(b)=app_button(&id) else {continue};
        let drag=gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::MOVE);
        let drag_id=id.clone();
        drag.connect_prepare(move |_,_,_|Some(gdk::ContentProvider::for_value(&drag_id.to_value())));
        b.add_controller(drag);
        let drop=gtk::DropTarget::new(String::static_type(),gdk::DragAction::MOVE);
        let current=order.clone();
        let target=area.clone();
        let destination=b.clone();
        let destination_id=id.clone();
        drop.connect_drop(move |_,value,_,_| {
            let Ok(source)=value.get::<String>() else{return false;};
            if source==destination_id{return false;}
            let updated={
                let mut items=current.borrow_mut();
                let Some(from)=items.iter().position(|s|s==&source) else{return false;};
                let Some(to)=items.iter().position(|s|s==&destination_id) else{return false;};
                let item=items.remove(from);
                items.insert(to,item);
                items.clone()
            };
            if let Err(error)=save_pins(&updated){
                eprintln!("Melania pin reorder: {error}");
                return false;
            }
            let mut children=Vec::new();
            let mut child=target.first_child();
            while let Some(widget)=child {child=widget.next_sibling();children.push(widget);}
            let mut previous:Option<gtk::Widget>=None;
            for key in updated {
                if let Some(widget)=children.iter().find(|widget|widget.widget_name().as_str()==key) {
                    target.reorder_child_after(widget,previous.as_ref());
                    previous=Some(widget.clone());
                }
            }
            destination.grab_focus();true
        });
        b.add_controller(drop);
        b.set_widget_name(&id);
        buttons.borrow_mut().push((id,b.clone()));
        area.append(&b);
    }
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
    let poll_path=path.clone();
    let refresh = {
        let target = box_running.clone();
        move |snapshot: Option<Vec<WindowInfo>>| {
            let Some(windows) = snapshot else {
                if last.borrow().is_some() {
                    for (_, button) in pinned_buttons.borrow().iter() {
                        button.set_visible(true);
                    }
                    while let Some(child) = target.first_child() {target.remove(&child);}
                    *last.borrow_mut() = None;
                }
                return;
            };
            // New pins may be added without any change to running windows.
            for (id,button) in pinned_buttons.borrow().iter() {
                let app_id=match id.as_str() {
                    "melania-files"=>"org.melaniadesktop.Files",
                    "melania-settings"=>"org.melaniadesktop.Settings",
                    _=>id.trim_end_matches(".desktop")
                };
                let running=windows.iter().any(|w|
                    w.app_id.trim_end_matches(".desktop").eq_ignore_ascii_case(app_id));
                button.set_visible(!running);
            }
            if last.borrow().as_ref() == Some(&windows) {return;}
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
                    let icon = taskbar_app_icon(&first.app_id);
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
                    let preview_inflight=Rc::new(Cell::new(false));
                    hover.connect_enter(move |_,_,_| {
                        menu_for_hover.popup();
                        // Never block GTK's event loop on synchronous compositor
                        // readback; a hover can request up to six previews.
                        if !use_thumbnails || preview_inflight.replace(true) {return;}
                        let Some(socket)=screenshot_endpoint.as_ref() else {
                            preview_inflight.set(false);
                            return;
                        };
                        // Only paths/IDs cross the worker boundary. GTK widgets
                        // always stay on the main thread.
                        let socket=socket.clone();
                        let ids:Vec<u64>=preview_targets.iter().take(6)
                            .map(|(id,_,_)|*id).collect();
                        let widgets:Vec<(Image,Picture)>=preview_targets.iter().take(6)
                            .map(|(_,icon,picture)|(icon.clone(),picture.clone())).collect();
                        let (sender,receiver)=std::sync::mpsc::channel::<(usize,PathBuf)>();
                        thread::spawn(move || {
                            for (index,id) in ids.into_iter().enumerate() {
                                if let Ok(image_path)=melania_control::request_preview(&socket,id) {
                                    if sender.send((index,image_path)).is_err(){break;}
                                }
                            }
                        });
                        let busy=preview_inflight.clone();
                        glib::timeout_add_local(Duration::from_millis(40),move ||{
                            match receiver.try_recv() {
                                Ok((index,image_path))=>{
                                    if let Some((icon,picture))=widgets.get(index) {
                                        picture.set_filename(Some(&image_path));
                                        picture.set_visible(true);
                                        icon.set_visible(false);
                                    }
                                    glib::ControlFlow::Continue
                                },
                                Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
                                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                                    busy.set(false);
                                    glib::ControlFlow::Break
                                }
                            }
                        });
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
                let window_icon = taskbar_app_icon(&window.app_id);
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
    // IPC can block while the compositor captures a window preview. Poll
    // on a bounded background worker: GTK only receives owned WindowInfo.
    let (sender,receiver)=std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        loop {
            let snapshot=poll_path.as_ref().and_then(|endpoint|
                control_request(endpoint,WindowCommand::List).ok());
            if sender.send(snapshot).is_err(){break;}
            thread::sleep(Duration::from_millis(400));
        }
    });
    glib::timeout_add_local(Duration::from_millis(75),move ||{
        match receiver.try_recv() {
            Ok(windows)=>{refresh(windows);glib::ControlFlow::Continue},
            Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected)=>glib::ControlFlow::Break,
        }
    });
    box_running
}

/// A single taskbar icon opens all configured virtual desktops.
/// The compositor owns the active-workspace state, not button-click guesses.
fn workspace_switcher() -> gtk::MenuButton {
    let toggle=gtk::MenuButton::new();
    toggle.add_css_class("melania-workspace-switcher");
    toggle.set_tooltip_text(Some("Desktop virtuali"));
    let socket=std::env::var_os(SOCKET_ENV).map(PathBuf::from);
    toggle.set_sensitive(socket.is_some());
    let light=melania_config::load().unwrap_or_default().appearance_theme()=="light";

    // Refined Task View glyph: two rounded miniature desktops rather than
    // offset squared frames. Drawn locally, consistent across icon themes.
    let symbol=gtk::DrawingArea::new();
    symbol.set_content_width(21);
    symbol.set_content_height(21);
    symbol.set_draw_func(move |_,cr,width,height| {
        if width<=0 || height<=0 {return;}
        cr.scale(width as f64/24.0,height as f64/24.0);
        let rounded = |x:f64,y:f64,w:f64,h:f64,r:f64| {
            cr.new_sub_path();
            cr.arc(x+w-r,y+r,r,-std::f64::consts::FRAC_PI_2,0.0);
            cr.arc(x+w-r,y+h-r,r,0.0,std::f64::consts::FRAC_PI_2);
            cr.arc(x+r,y+h-r,r,std::f64::consts::FRAC_PI_2,std::f64::consts::PI);
            cr.arc(x+r,y+r,r,std::f64::consts::PI,3.0*std::f64::consts::FRAC_PI_2);
            cr.close_path();
        };
        cr.set_line_width(1.65);
        // Two landscape desktop thumbnails with understated monitor feet.
        rounded(1.2,6.0,10.0,8.5,1.6);
        cr.set_source_rgba(0.33,0.68,0.96,if light {0.28} else {0.46});
        let _=cr.fill_preserve();
        if light {cr.set_source_rgb(0.19,0.33,0.47);}
        else {cr.set_source_rgb(0.94,0.97,1.0);}
        let _=cr.stroke();
        rounded(12.8,6.0,10.0,8.5,1.6);
        let _=cr.stroke();
        cr.set_line_width(1.5);
        cr.move_to(4.0,17.5);
        cr.line_to(8.5,17.5);
        cr.move_to(15.6,17.5);
        cr.line_to(20.1,17.5);
        let _=cr.stroke();
    });
    let active_label=Label::new(Some("–"));
    active_label.add_css_class("melania-workspace-current");
    let content=GtkBox::new(Orientation::Horizontal,3);
    content.add_css_class("melania-workspace-switcher-content");
    content.append(&symbol);
    content.append(&active_label);
    toggle.set_child(Some(&content));

    let popover=gtk::Popover::new();
    // The switcher sits in the bottom bar: the selector must open upwards.
    popover.set_position(gtk::PositionType::Top);
    popover.add_css_class("melania-workspace-popover");
    let list=GtkBox::new(Orientation::Vertical,3);
    list.add_css_class("melania-workspace-menu");
    list.append(&label("DESKTOP VIRTUALI","melania-workspace-heading"));
    let mut choices=Vec::new();
    let count=melania_config::load().unwrap_or_default().workspace_count;
    for n in 1..=count {
        let choice=Button::with_label(&format!("Desktop {n}"));
        choice.add_css_class("melania-workspace-choice");
        choice.set_sensitive(socket.is_some());
        let path=socket.clone();
        let popup=popover.clone();
        choice.connect_clicked(move |_| {
            if let Some(path)=path.as_ref() {
                if let Err(error)=control_request(path,WindowCommand::Workspace(n)) {
                    eprintln!("Melania workspace {n}: {error}");
                }
            }
            popup.popdown();
        });
        list.append(&choice);
        choices.push(choice);
    }
    popover.set_child(Some(&list));
    toggle.set_popover(Some(&popover));
    let refresh_toggle=toggle.clone();
    let refresh_label=active_label.clone();
    let refresh=move || {
        let Some(path)=socket.as_ref() else{return};
        if let Ok(active)=current_workspace(path) {
            refresh_label.set_label(&active.to_string());
            refresh_toggle.set_tooltip_text(Some(&format!("Desktop {active} · Cambia desktop")));
            for (index,choice) in choices.iter().enumerate() {
                if (index+1) as u8==active {
                    choice.add_css_class("melania-workspace-active");
                } else {
                    choice.remove_css_class("melania-workspace-active");
                }
            }
        }
    };
    refresh();
    glib::timeout_add_local(Duration::from_millis(900),move ||{
        refresh();glib::ControlFlow::Continue
    });
    toggle
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
    let logo=if gdk::Display::default()
        .map(|display| gtk::IconTheme::for_display(&display).has_icon("melania"))
        .unwrap_or(false) {
        Image::from_icon_name("melania")
    } else {
        Image::from_file(ICON)
    };
    logo.set_pixel_size(30);menu.set_child(Some(&logo));menu.set_tooltip_text(Some("Menu Melania"));
    menu.set_size_request(43,43); // only Start is circular; other task buttons stay rectangular.
    let sm=start.clone();let qs=quick.clone();let nt=notifications.clone();
    menu.connect_clicked(move |_|{hide_popup_surface(&nt);toggle_surface(&sm,&qs);});
    let start_button=menu.clone();
    start.connect_notify_local(Some("visible"),move |surface,_| {
        if surface.is_visible(){start_button.add_css_class("melania-start-open");}
        else{start_button.remove_css_class("melania-start-open");}
    });
    bar.append(&menu);
    // All installed .desktop applications are pinnable. Changes from Start
    // appear automatically in the taskbar without restarting the session.
    let pinned=GtkBox::new(Orientation::Horizontal,6);
    let pinned_buttons:Rc<RefCell<Vec<(String,Button)>>>=Rc::new(RefCell::new(Vec::new()));
    let pinned_order:Rc<RefCell<Vec<String>>>=Rc::new(RefCell::new(Vec::new()));
    populate_pins(&pinned,&pinned_buttons,&pinned_order);
    let pinned_area=pinned.clone();
    let pinned_widgets=pinned_buttons.clone();
    let pinned_ids=pinned_order.clone();
    let pins_file=pins_path();
    let mut pins_stamp=pins_file.as_deref().and_then(file_stamp);
    glib::timeout_add_local(Duration::from_millis(800),move ||{
        let updated=pins_file.as_deref().and_then(file_stamp);
        if updated!=pins_stamp {
            pins_stamp=updated;
            if load_pins()!=*pinned_ids.borrow(){
                populate_pins(&pinned_area,&pinned_widgets,&pinned_ids);
            }
        }
        glib::ControlFlow::Continue
    });
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
    let space=GtkBox::new(Orientation::Horizontal,1);space.set_hexpand(true);bar.append(&space);
    // One compact trailing cluster keeps tray, notifications and clock close together
    // regardless of panel width, position or the number of running windows.
    let trailing=GtkBox::new(Orientation::Horizontal,2);
    trailing.add_css_class("melania-trailing");
    // Do not let GTK give the system area the spare width of the taskbar.
    // Otherwise the clock button stretches and leaves a very wide blank
    // region between the notification hairline and the right-aligned time.
    trailing.set_hexpand(false);
    trailing.set_halign(gtk::Align::End);
    // Center the entire system cluster vertically at both panel positions.
    trailing.set_valign(gtk::Align::Center);
    bar.append(&trailing);
    // Keep the desktop switcher beside the tray instead of centered on the bar.
    trailing.append(&workspace_switcher());
    // Exact Melania compact symbolic glyphs: no glossy 96px app icon tiles.
    let status=Button::new();status.add_css_class("melania-system");
    status.add_css_class("melania-tray-button");
    status.set_tooltip_text(Some("Rete, Bluetooth, audio e alimentazione"));
    let status_row=GtkBox::new(Orientation::Horizontal,3);
    status_row.add_css_class("melania-status-icons");
    let light_tray=melania_config::load().unwrap_or_default().appearance_theme()=="light";
    for (icon_name, icon_class) in [
        ("melania-tray-wifi", "melania-status-wifi"),
        ("melania-tray-bluetooth", "melania-status-bluetooth"),
        ("melania-tray-volume", "melania-status-volume"),
        ("melania-tray-battery", "melania-status-battery"),
    ] {
        // These are real SVG images, not GTK -symbolic masks; the latter
        // flattened the Bluetooth rune into an opaque circle.
        let asset=if light_tray { format!("{icon_name}-light") } else { icon_name.to_string() };
        let fallback=match icon_name {
            "melania-tray-wifi" => "network-wireless",
            "melania-tray-bluetooth" => "bluetooth-active",
            "melania-tray-volume" => "audio-volume-high",
            "melania-tray-battery" => "battery-full",
            _ => "dialog-information",
        };
        let icon=icon_with_fallback(&asset,fallback);
        icon.set_pixel_size(22);
        icon.add_css_class("melania-tray-symbol");
        icon.add_css_class(icon_class);
        status_row.append(&icon);
    }
    status.set_child(Some(&status_row));
    let qs=quick.clone();let sm=start.clone();let nt=notifications.clone();
    status.connect_clicked(move |_|{hide_popup_surface(&nt);toggle_surface(&qs,&sm);});trailing.append(&status);
    let notification_button=Button::new();
    notification_button.set_tooltip_text(Some("Centro notifiche"));
    notification_button.add_css_class("melania-tray-button");
    let notification_row=GtkBox::new(Orientation::Horizontal,4);
    let notification_asset=if light_tray { "melania-tray-notifications-light" } else { "melania-tray-notifications" };
    let notice_icon=icon_with_fallback(notification_asset,"preferences-system-notifications");
    notice_icon.set_pixel_size(22);
    notice_icon.add_css_class("melania-tray-symbol");
    notification_row.append(&notice_icon);
    let badge=Label::new(None);badge.add_css_class("melania-notice-count");
    // An empty badge previously rendered as a thick blue vertical stripe.
    badge.set_visible(false);
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
                    badge.set_visible(unread>0);
                    previous_unread=unread;
                }
            }
        }
        glib::ControlFlow::Continue
    });
    notification_button.add_css_class("melania-system");
    let notify=notifications.clone();let sm=start.clone();let qs=quick.clone();
    notification_button.connect_clicked(move |_|{
        hide_start_surface(&sm);hide_popup_surface(&qs);
        if popup_is_open(&notify){hide_popup_surface(&notify);}else{present_popup_surface(&notify);}
    });
    trailing.append(&notification_button);
    // Real GTK signal regression reproducing the user's failure: open the
    // Wi-Fi/Bluetooth quick surface, then click the adjacent notifications
    // button. Smithay must keep the panel layer mapped and alive.
    if std::env::var_os("MELANIA_SMOKE_NOTIFICATION_SWITCH")
        .as_deref() == Some(std::ffi::OsStr::new("1")) {
        let q=quick.clone();
        let click=notification_button.clone();
        glib::timeout_add_local_once(Duration::from_millis(850),move || {
            present_popup_surface(&q);
            glib::timeout_add_local_once(Duration::from_millis(500),move || {
                click.emit_by_name::<()>("clicked",&[]);
                eprintln!("MELANIA_SMOKE_NOTIFICATION_CLICK_OK");
            });
        });
    }
    // Replace the accidental double-blue notification stripe with one
    // intentional hairline before the Windows 11-style vertical clock.
    let clock_separator=gtk::Separator::new(Orientation::Vertical);
    clock_separator.add_css_class("melania-clock-separator");
    trailing.append(&clock_separator);
    // Time above date, right-aligned, in the far-right taskbar control.
    let clock=Button::new();
    clock.add_css_class("melania-system");
    clock.add_css_class("melania-clock");
    // Never stretch the clock to fill the trailing cluster: use only its
    // natural two-line text width plus minimal horizontal hit-target padding.
    clock.set_hexpand(false);
    clock.set_halign(gtk::Align::Start);
    // GTK must not stretch or top-align the two-line clock within a tall bar.
    clock.set_valign(gtk::Align::Center);
    let clock_row=GtkBox::new(Orientation::Vertical,0);
    clock_row.add_css_class("melania-clock-row");
    // A shared width and fill alignment make both strings end at the exact
    // same x coordinate, as in the user's Windows clock reference.
    clock_row.set_size_request(84,-1);
    clock_row.set_halign(gtk::Align::End);
    clock_row.set_valign(gtk::Align::Center);
    clock_row.set_vexpand(false);
    let date_label=Label::new(None);
    date_label.add_css_class("melania-clock-date");
    date_label.set_halign(gtk::Align::Fill);
    date_label.set_hexpand(true);
    date_label.set_xalign(1.0);
    let time_label=Label::new(None);
    time_label.add_css_class("melania-clock-time");
    time_label.set_halign(gtk::Align::Fill);
    time_label.set_hexpand(true);
    time_label.set_xalign(1.0);
    clock_row.append(&time_label);
    clock_row.append(&date_label);
    clock.set_child(Some(&clock_row));
    let update=move || {
        let now=glib::DateTime::now_local().ok();
        let date=now.as_ref().and_then(|t|t.format("%d/%m/%Y").ok());
        let hour=now.as_ref().and_then(|t|t.format("%H:%M").ok());
        date_label.set_text(date.as_deref().unwrap_or("--/--/----"));
        time_label.set_text(hour.as_deref().unwrap_or("--:--"));
    };
    update();
    glib::timeout_add_seconds_local(20,move ||{update();glib::ControlFlow::Continue});
    let cal=calendar.clone();let qs=quick.clone();let sm=start.clone();let nt=notifications.clone();
    clock.connect_clicked(move |_|{
        hide_popup_surface(&qs);hide_start_surface(&sm);hide_popup_surface(&nt);
        if popup_is_open(&cal){hide_popup_surface(&cal);}else{present_popup_surface(&cal);}
    });
    trailing.append(&clock);
    w.set_child(Some(&bar));w
}
/// Local private datagram channel: the compositor alone owns the Super key,
/// while the panel alone owns the Start window's GTK state. A per-compositor
/// control-socket-derived name keeps nested sessions independent.
fn install_start_hotkey_listener(
    app: &Application,
    start: &ApplicationWindow,
    quick: &ApplicationWindow,
    notifications: &ApplicationWindow,
    calendar: &ApplicationWindow,
) {
    let Some(control)=std::env::var_os(SOCKET_ENV).filter(|v|!v.is_empty()) else { return; };
    let path=start_toggle_socket_path(&PathBuf::from(control));
    // Never follow a symlink or replace a live listener. The parent is the
    // user's private 0700 XDG_RUNTIME_DIR.
    if let Ok(meta)=fs::symlink_metadata(&path) {
        if !meta.file_type().is_socket() {
            eprintln!("Melania Start hotkey: unexpected socket path {:?}", path);
            return;
        }
        if UnixDatagram::unbound().is_ok_and(|sock|sock.connect(&path).is_ok()) {
            eprintln!("Melania Start hotkey: listener already running");
            return;
        }
        let _=fs::remove_file(&path);
    }
    let socket=match UnixDatagram::bind(&path) {
        Ok(socket)=>socket,
        Err(error)=>{eprintln!("Melania Start hotkey socket: {error}");return;},
    };
    if let Err(error)=fs::set_permissions(&path,fs::Permissions::from_mode(0o600)) {
        eprintln!("Melania Start hotkey permissions: {error}");
        let _=fs::remove_file(&path);
        return;
    }
    if let Err(error)=socket.set_nonblocking(true) {
        eprintln!("Melania Start hotkey nonblocking: {error}");
        let _=fs::remove_file(&path);
        return;
    }
    let cleanup_path=path.clone();
    app.connect_shutdown(move |_| { let _=fs::remove_file(&cleanup_path); });
    let open=start.clone();
    let quick=quick.clone();
    let notifications=notifications.clone();
    let calendar=calendar.clone();
    glib::timeout_add_local(Duration::from_millis(30),move || {
        let mut packet=[0u8;32];
        // Bounded work per UI tick even if a sender floods the local socket.
        for _ in 0..8 {
            match socket.recv(&mut packet) {
                Ok(n) if &packet[..n]==b"TOGGLE_START"=>{
                    hide_popup_surface(&notifications);
                    toggle_surface(&open,&quick);
                    if std::env::var_os("MELANIA_SMOKE_OPEN_START").is_some() {
                        eprintln!("MELANIA_START_IPC toggled open={}",open.is_visible() && !open.has_css_class("melania-start-concealed"));
                    }
                }
                Ok(n) if &packet[..n]==b"CLOSE_START" => hide_start_surface(&open),
                Ok(n) if &packet[..n]==b"CLOSE_OVERLAYS" => {
                    let was_open=open.is_visible() && !open.has_css_class("melania-start-concealed") ||
                        [&quick,&notifications,&calendar].iter().any(|window|popup_is_open(window));
                    // Keep each already-realized layer role alive: only conceal.
                    hide_start_surface(&open);
                    hide_popup_surface(&quick);
                    hide_popup_surface(&notifications);
                    hide_popup_surface(&calendar);
                    if std::env::var_os("MELANIA_SMOKE_OUTSIDE_CLICK").as_deref() ==
                        Some(std::ffi::OsStr::new("1")) {
                        let still_open=open.is_visible() && !open.has_css_class("melania-start-concealed") ||
                            [&quick,&notifications,&calendar].iter().any(|window|popup_is_open(window));
                        eprintln!("MELANIA_OUTSIDE_CLICK_DISMISSED previously_open={was_open} now_open={still_open}");
                    }
                },
                Ok(_)=>{},
                Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>break,
                Err(error)=>{eprintln!("Melania Start hotkey receive: {error}");break;},
            }
        }
        glib::ControlFlow::Continue
    });
}

fn build(app:&Application){
    // Measure actual GTK construction costs to guide further startup work.
    // Measurements refer to activation, not total compositor boot time.
    let startup = Instant::now();
    setup_melania_icon_theme();
    let theme_ready = startup.elapsed();
    if !gtk4_layer_shell::is_supported() {
        eprintln!("Melania Panel requires Wayland with zwlr_layer_shell_v1 (try the nested compositor)");
        return;
    }
    let preferences = melania_config::load().unwrap_or_default();
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_property("gtk-enable-animations",
            preferences.animations && !preferences.reduce_motion);
        gtk_settings.set_property("gtk-application-prefer-dark-theme", preferences.dark_mode);
    }
    let css=gtk::CssProvider::new();css.load_from_data(include_str!("../assets/panel.css"));
    if let Some(display)=gdk::Display::default(){gtk::style_context_add_provider_for_display(&display,&css,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);}
    let start=start_window(app);
    let start_ready = startup.elapsed();
    let notifications=notifications_window(app);
    let notifications_ready = startup.elapsed();
    let quick=quick_window(app,&notifications);
    let quick_ready = startup.elapsed();
    let calendar=calendar_window(app);
    let calendar_ready = startup.elapsed();
    install_start_hotkey_listener(app,&start,&quick,&notifications,&calendar);
    // Do not present popup windows until the user clicks.
    let bar=taskbar(app,&start,&quick,&notifications,&calendar);
    let taskbar_ready = startup.elapsed();
    if let Some(content)=bar.child().and_then(|child|child.downcast::<GtkBox>().ok()) {
        panel_preferences::attach_live_preferences(
            &bar,&content,&start,&quick,&notifications,&calendar);
    }
    bar.present();
    if std::env::var_os("MELANIA_PROFILE_START").is_some() {
        eprintln!(
            "MELANIA_PANEL_BOOT_MS theme={} start={} notifications={} quick={} calendar={} taskbar={} present={} total={}",
            theme_ready.as_millis(),
            (start_ready-theme_ready).as_millis(),
            (notifications_ready-start_ready).as_millis(),
            (quick_ready-notifications_ready).as_millis(),
            (calendar_ready-quick_ready).as_millis(),
            (taskbar_ready-calendar_ready).as_millis(),
            (startup.elapsed()-taskbar_ready).as_millis(),
            startup.elapsed().as_millis()
        );
    }
    // CI-only: show the *real* launcher in the nested compositor screenshot.
    // Never open Start automatically in a normal user session.
    if std::env::var_os("MELANIA_SMOKE_OPEN_START").as_deref() == Some(std::ffi::OsStr::new("1")) {
        start.present();
    }
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
    let selected_theme=melania_config::resolved_icon_theme(&prefs);
    eprintln!("MELANIA_PANEL_ICON_THEME_SELECTED={selected_theme}");
    if let Some(settings)=gtk::Settings::default() {
        settings.set_property("gtk-icon-theme-name",selected_theme);
    }
}

fn main()->glib::ExitCode {
    let app=Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);app.run()
}
