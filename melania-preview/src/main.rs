//! Melania desktop concept: real GTK controls, but not yet a Wayland shell.
use gtk::prelude::*;
use gtk::{gdk, gio};
use melania_transfer as transfer;
use gtk::{Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Image, Label, Orientation, Overlay, Scale, Switch};
use std::{cell::{Cell, RefCell}, path::PathBuf, process::Command, rc::Rc};
use melania_config::{self, Preferences};
use melania_services as services;

const APP_ID: &str = "org.melaniadesktop.Preview";
const LOGO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/melania-icon.png");
const START_ICON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/melania-start.png");

fn desktop_directory() -> Option<PathBuf> {
    // Respect localized XDG desktop directories, e.g. ~/Scrivania.
    if let Ok(output) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if output.status.success() {
            let dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            if dir.is_dir() { return Some(dir); }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    [home.join("Scrivania"), home.join("Desktop")]
        .into_iter().find(|p| p.is_dir())
}

fn refresh_desktop_items(items: &GtkBox) {
    while let Some(child) = items.first_child() { items.remove(&child); }
    let Some(directory) = desktop_directory() else {
        items.append(&title("Cartella Scrivania non disponibile", "wallpaper-note"));
        return;
    };
    let Ok(read) = std::fs::read_dir(directory) else { return; };
    let mut paths = read.filter_map(Result::ok).map(|e| e.path()).collect::<Vec<_>>();
    paths.sort_by_key(|path| path.file_name().unwrap_or_default().to_string_lossy().to_lowercase());
    for path in paths.iter().take(9) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let button = Button::with_label(&format!("{}  {name}", if path.is_dir() { "📁" } else { "📄" }));
        button.add_css_class("desktop-icon");
        button.set_halign(Align::Start);
        let open_path = path.clone();
        button.connect_clicked(move |_| {
            if let Err(error) = Command::new("xdg-open").arg(&open_path).spawn() {
                eprintln!("Melania desktop: {error}");
            }
        });
        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::COPY | gdk::DragAction::MOVE);
        let initial = Rc::new(RefCell::new(None));
        let saved = initial.clone();
        let origin = path.clone();
        drag.connect_prepare(move |_, _, _| {
            *saved.borrow_mut() = Some(transfer::source_identity(&origin).ok()?);
            let list = gdk::FileList::from_array(&[gio::File::for_path(&origin)]);
            Some(gdk::ContentProvider::for_value(&list.to_value()))
        });
        let moved_path = path.clone();
        let reload = items.clone();
        drag.connect_drag_end(move |_, _, delete_data| {
            let identity = initial.borrow_mut().take();
            if delete_data {
                if let Some(identity) = identity {
                    if let Err(error) = transfer::remove_confirmed_source(&moved_path, &identity) {
                        eprintln!("Melania desktop: origine non cancellata: {error}");
                    }
                }
                refresh_desktop_items(&reload);
            }
        });
        button.add_controller(drag);
        items.append(&button);
    }
    if paths.len() > 9 {
        items.append(&title(&format!("… e altri {} file", paths.len() - 9), "wallpaper-note"));
    }
}

fn enable_desktop_drop(desktop: &GtkBox, feedback: &Label, desktop_items: &GtkBox) {
    let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY | gdk::DragAction::MOVE);
    let feedback = feedback.clone();
    let desktop_items = desktop_items.clone();
    let highlight = desktop.clone();
    target.connect_enter(move |_, _, _| {
        highlight.add_css_class("desktop-drop-ready");
        gdk::DragAction::COPY
    });
    let highlight = desktop.clone();
    target.connect_leave(move |_| highlight.remove_css_class("desktop-drop-ready"));
    target.connect_drop(move |_, value, _, _| {
        let Some(dest) = desktop_directory() else {
            feedback.set_text("Cartella Desktop non trovata: configura la directory XDG DESKTOP.");
            return false;
        };
        let Ok(list) = value.get::<gdk::FileList>() else { return false; };
        let mut sources = Vec::new();
        for file in list.files() {
            let Some(path) = file.path() else {
                feedback.set_text("Il trasferimento da posizioni remote non è ancora supportato.");
                return false;
            };
            sources.push(path);
        }
        match transfer::copy_sources(&sources, &dest) {
            Ok(created) => {
                feedback.set_text(&format!("{} file copiati sulla Scrivania ({})", created.len(), dest.display()));
                refresh_desktop_items(&desktop_items);
                true
            },
            Err(error) => {
                feedback.set_text(&format!("Drop annullato: {error}"));
                false
            }
        }
    });
    desktop.add_controller(target);
}

fn title(s: &str, style: &str) -> Label {
    let label = Label::new(Some(s));
    label.set_xalign(0.0);
    label.add_css_class(style);
    label
}

fn launch(program: &str, args: &[&str]) {
    if let Err(e) = Command::new(program).args(args).spawn() {
        eprintln!("Melania: impossibile avviare {program}: {e}");
    }
}

fn start_menu() -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 10);
    panel.add_css_class("melania-panel");
    panel.set_size_request(380, 520);
    panel.set_halign(Align::Start);
    panel.set_valign(Align::End);
    panel.set_margin_start(14);
    panel.set_margin_bottom(70);
    panel.append(&title("Melania — Applicazioni", "melania-panel-title"));
    let search = Entry::builder().placeholder_text("Cerca applicazioni installate...").build();
    panel.append(&search);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_min_content_height(340);
    let app_list = GtkBox::new(Orientation::Vertical, 3);
    let mut buttons = Vec::new();
    // We list applications registered through the XDG/GIO database rather than
    // hard-coding Firefox, terminal, etc. Works for GTK, Qt and other apps.
    let mut apps = gtk::gio::AppInfo::all().into_iter()
        .filter(|app| app.should_show())
        .collect::<Vec<_>>();
    apps.sort_by_key(|app| app.name().to_string().to_lowercase());
    for app in apps {
        let name = app.name().to_string();
        let button = Button::with_label(&name);
        button.set_halign(Align::Fill);
        button.add_css_class("launcher-row");
        button.connect_clicked(move |_| {
            if let Err(err) = app.launch(&[], None::<&gtk::gio::AppLaunchContext>) {
                eprintln!("Melania launcher: {err}");
            }
        });
        app_list.append(&button);
        buttons.push((button, name.to_lowercase()));
    }
    for (name, executable) in [("Melania Files", "melania-files"), ("Impostazioni", "melania-settings")] {
        let button = Button::with_label(name);
        button.set_halign(Align::Fill);
        button.add_css_class("launcher-row");
        button.connect_clicked(move |_| launch(executable, &[]));
        app_list.prepend(&button);
        buttons.push((button, name.to_lowercase()));
    }
    search.connect_changed(move |entry| {
        let q = entry.text().to_lowercase();
        for (button, name) in &buttons { button.set_visible(name.contains(&q)); }
    });
    scroll.set_child(Some(&app_list));
    panel.append(&scroll);
    panel.append(&title("Applicazioni rilevate dal sistema XDG/GIO", "melania-hint"));
    panel
}

fn binary_toggle(
    parent: &GtkBox,
    label: &'static str,
    getter: fn() -> services::Result<bool>,
    setter: fn(bool) -> services::Result<()>,
    feedback: &Label,
) {
    let current = getter().ok();
    let state = Rc::new(Cell::new(current));
    let caption = match current { Some(true) => "Attivo", Some(false) => "Spento", None => "Non disponibile" };
    let button = Button::with_label(&format!("{label} · {caption}"));
    button.add_css_class("quick-action");
    let state_copy = state.clone();
    let feedback = feedback.clone();
    button.connect_clicked(move |button| {
        // Read actual system state before toggling, never trust stale UI.
        let result = getter().and_then(|on| setter(!on).and_then(|_| getter()));
        match result {
            Ok(active) => {
                state_copy.set(Some(active));
                button.set_label(&format!("{label} · {}", if active { "Attivo" } else { "Spento" }));
                feedback.set_text(&format!("{label}: stato aggiornato"));
            }
            Err(err) => feedback.set_text(&format!("{label}: {err}")),
        }
    });
    if state.get().is_none() { button.set_sensitive(false); }
    parent.append(&button);
}

fn quick_settings(window: &ApplicationWindow, prefs: Rc<RefCell<Preferences>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 10);
    panel.add_css_class("melania-panel");
    panel.set_size_request(338, 430);
    panel.set_halign(Align::End);
    panel.set_valign(Align::End);
    panel.set_margin_end(14);
    panel.set_margin_bottom(70);
    panel.append(&title("Centro rapido", "melania-panel-title"));
    panel.append(&title("Controlli del dispositivo", "melania-hint"));
    let feedback = title("Stato rilevato dai servizi Linux.", "melania-hint");
    feedback.set_wrap(true);
    binary_toggle(&panel, "Wi-Fi", services::wifi_enabled, services::set_wifi, &feedback);
    binary_toggle(&panel, "Bluetooth", services::bluetooth_enabled, services::set_bluetooth, &feedback);
    let power_row = GtkBox::new(Orientation::Horizontal, 8);
    let balanced = Button::with_label("Bilanciato");
    let saver = Button::with_label("Risparmio energia");
    let feedback_b = feedback.clone();
    balanced.connect_clicked(move |_| {
        match services::set_power_profile(services::PowerProfile::Balanced) {
            Ok(()) => feedback_b.set_text("Profilo bilanciato richiesto."),
            Err(e) => feedback_b.set_text(&e.to_string()),
        }
    });
    let feedback_s = feedback.clone();
    saver.connect_clicked(move |_| {
        match services::set_power_profile(services::PowerProfile::PowerSaver) {
            Ok(()) => feedback_s.set_text("Risparmio energetico richiesto."),
            Err(e) => feedback_s.set_text(&e.to_string()),
        }
    });
    power_row.append(&balanced); power_row.append(&saver);
    panel.append(&power_row);

    let prefs_dnd = prefs.clone();
    let dnd = Button::with_label(if prefs.borrow().do_not_disturb { "Non disturbare · Sì" } else { "Non disturbare · No" });
    let status_dnd = feedback.clone();
    dnd.connect_clicked(move |button| {
        let mut p = prefs_dnd.borrow_mut();
        p.do_not_disturb = !p.do_not_disturb;
        button.set_label(if p.do_not_disturb { "Non disturbare · Sì" } else { "Non disturbare · No" });
        match melania_config::save(&p) {
            Ok(()) => status_dnd.set_text("Preferenza notifiche salvata (servizio notifiche non ancora collegato)."),
            Err(e) => status_dnd.set_text(&format!("Salvataggio fallito: {e}")),
        }
    });
    panel.append(&dnd);
    let dark = gtk::Switch::builder().active(prefs.borrow().dark_mode).build();
    let row = GtkBox::new(Orientation::Horizontal, 10);
    let label = title("Tema scuro", "");
    label.set_hexpand(true);
    row.append(&label); row.append(&dark);
    panel.append(&row);
    let win = window.clone();
    let pref_dark = prefs.clone();
    let feedback_dark = feedback.clone();
    dark.connect_active_notify(move |s| {
        let active = s.is_active();
        if active { win.remove_css_class("melania-light"); }
        else { win.add_css_class("melania-light"); }
        let mut p = pref_dark.borrow_mut();
        p.dark_mode = active;
        if let Err(e) = melania_config::save(&p) {
            feedback_dark.set_text(&format!("Impossibile salvare il tema: {e}"));
        }
    });

    for (name, initial, setter) in [
        ("Volume", services::volume().ok(), services::set_volume as fn(u8)->services::Result<()>),
        ("Luminosità", services::brightness().ok(), services::set_brightness as fn(u8)->services::Result<()>),
    ] {
        panel.append(&title(name, "melania-hint"));
        let h = GtkBox::new(Orientation::Horizontal, 8);
        let slider = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        slider.set_hexpand(true);
        slider.set_value(initial.unwrap_or(50) as f64);
        slider.set_sensitive(initial.is_some());
        let apply = Button::with_label("Applica");
        apply.set_sensitive(initial.is_some());
        let status = feedback.clone();
        let value_slider = slider.clone();
        apply.connect_clicked(move |_| {
            let pct = value_slider.value().round().clamp(0.0, 100.0) as u8;
            match setter(pct) {
                Ok(()) => status.set_text(&format!("{name}: {pct}% impostato")),
                Err(e) => status.set_text(&format!("{name}: {e}")),
            }
        });
        h.append(&slider); h.append(&apply);
        panel.append(&h);
    }
    panel.append(&feedback);
    panel
}

fn build_ui(app: &Application) {
    let provider = gtk::CssProvider::new();
    #[allow(deprecated)]
    provider.load_from_data(include_str!("../assets/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let window = ApplicationWindow::builder()
        .application(app).title("Melania Desktop — anteprima 0.4")
        .default_width(1150).default_height(700).build();
    let overlay = Overlay::new();
    let base = GtkBox::new(Orientation::Vertical, 0);
    base.add_css_class("desktop");
    let desktop = GtkBox::new(Orientation::Vertical, 16);
    desktop.set_vexpand(true);
    desktop.set_margin_start(32);
    desktop.set_margin_top(26);
    let brand = GtkBox::new(Orientation::Horizontal, 12);
    let logo = Image::from_file(LOGO);
    logo.set_pixel_size(65);
    brand.append(&logo);
    let name_box = GtkBox::new(Orientation::Vertical, 3);
    name_box.append(&title("MELANIA", "desktop-brand"));
    name_box.append(&title("Il desktop familiare, ripensato.", "desktop-sub"));
    brand.append(&name_box);
    desktop.append(&brand);
    desktop.append(&title("Melania 0.4 · trascina qui i file per copiarli sulla Scrivania", "wallpaper-note"));
    let icons = GtkBox::new(Orientation::Vertical, 10);
    icons.set_halign(Align::Start);
    for (text, command, args) in [
        ("⌂  Home", "xdg-open", "."),
        ("▣  File", "melania-files", ""),
        ("⚙  Impostazioni", "melania-settings", ""),
    ] {
        let button = Button::with_label(text);
        button.add_css_class("desktop-icon");
        let exe = command.to_string();
        let arg = args.to_string();
        button.connect_clicked(move |_| {
            if arg.is_empty() { launch(&exe, &[]); } else { launch(&exe, &[&arg]); }
        });
        icons.append(&button);
    }
    desktop.append(&icons);
    let desktop_items = GtkBox::new(Orientation::Vertical, 6);
    desktop_items.set_halign(Align::Start);
    desktop_items.add_css_class("desktop-file-icons");
    desktop.append(&desktop_items);
    refresh_desktop_items(&desktop_items);
    let drop_feedback = title("Drag & drop attivo nella demo · non sostituisce il desktop del sistema", "wallpaper-note");
    desktop.append(&drop_feedback);
    enable_desktop_drop(&desktop, &drop_feedback, &desktop_items);
    base.append(&desktop);
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("melania-taskbar");
    let start = Button::new();
    start.add_css_class("melania-start-btn");
    let start_icon = Image::from_file(START_ICON);
    start_icon.set_pixel_size(31);
    start.set_child(Some(&start_icon));
    start.set_tooltip_text(Some("Apri menu Melania"));
    bar.append(&start);
    for (label, exe, args) in [
        ("▣  File", "melania-files", ""),
        ("⌘  Browser", "xdg-open", "https://www.example.com"),
        ("⚙  Impostazioni", "melania-settings", ""),
    ] {
        let b = Button::with_label(label);
        let e = exe.to_string(); let a = args.to_string();
        b.connect_clicked(move |_| { if a.is_empty() { launch(&e, &[]); } else { launch(&e, &[&a]); } });
        bar.append(&b);
    }
    let filler = GtkBox::new(Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    bar.append(&filler);
    let quick = Button::with_label("⌁  Wi-Fi  •  Bluetooth  •  Audio     --:--");
    let clock_button = quick.clone();
    let update_clock = move || {
        let now = gtk::glib::DateTime::now_local()
            .ok()
            .and_then(|dt| dt.format("%H:%M").ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "--:--".to_string());
        clock_button.set_label(&format!("⌁  Wi-Fi  •  Bluetooth  •  Audio     {now}"));
    };
    update_clock();
    gtk::glib::timeout_add_seconds_local(30, move || {
        update_clock();
        gtk::glib::ControlFlow::Continue
    });
    quick.set_tooltip_text(Some("Apri centro rapido Melania"));
    bar.append(&quick);
    base.append(&bar);
    overlay.set_child(Some(&base));
    let menu = start_menu();
    menu.set_visible(false);
    overlay.add_overlay(&menu);
    let prefs = Rc::new(RefCell::new(melania_config::load().unwrap_or_default()));
    if !prefs.borrow().dark_mode { window.add_css_class("melania-light"); }
    let settings = quick_settings(&window, prefs);
    settings.set_visible(false);
    overlay.add_overlay(&settings);
    let menu_clone = menu.clone();
    let settings_clone = settings.clone();
    start.connect_clicked(move |_| {
        settings_clone.set_visible(false);
        menu_clone.set_visible(!menu_clone.is_visible());
    });
    let menu_clone = menu.clone();
    let settings_clone = settings.clone();
    quick.connect_clicked(move |_| {
        menu_clone.set_visible(false);
        settings_clone.set_visible(!settings_clone.is_visible());
    });
    window.set_child(Some(&overlay));
    window.present();
}

fn main() -> gtk::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
