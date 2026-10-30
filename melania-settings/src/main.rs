//! Melania control centre: saved desktop preferences and system status.
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Entry, HeaderBar, Image, Label, Orientation, Scale, SpinButton, Stack, Switch};
use std::{cell::RefCell, rc::Rc};
use melania_config::{self, Preferences};

fn text(label: &str, style: &str) -> Label {
    let widget = Label::new(Some(label));
    widget.set_xalign(0.0);
    widget.set_wrap(true);
    widget.add_css_class(style);
    widget
}

fn saved_option(
    title: &str,
    description: &str,
    current: bool,
    prefs: Rc<RefCell<Preferences>>,
    change: fn(&mut Preferences, bool),
    after_change: Option<Rc<dyn Fn(&Preferences)>>,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 18);
    row.add_css_class("settings-card");
    let v = GtkBox::new(Orientation::Vertical, 5);
    v.set_hexpand(true);
    v.append(&text(title, "settings-hero"));
    v.append(&text(description, "settings-subtitle"));
    row.append(&v);
    let toggle = Switch::builder().active(current).build();
    // Do not allow GTK's fill alignment to stretch the thumb vertically.
    toggle.add_css_class("settings-switch");
    toggle.set_valign(gtk::Align::Center);
    toggle.set_halign(gtk::Align::End);
    toggle.set_hexpand(false);
    toggle.set_vexpand(false);
    toggle.set_size_request(54, 30);
    toggle.set_tooltip_text(Some(title));
    toggle.connect_active_notify(move |sw| {
        let mut preferences = prefs.borrow_mut();
        change(&mut preferences, sw.is_active());
        if let Err(e) = melania_config::save(&preferences) {
            eprintln!("Melania Settings: impossibile salvare preferenze: {e}");
        }
        if let Some(ref update) = after_change {
            update(&preferences);
        }
    });
    row.append(&toggle);
    row
}

fn page(title: &str, description: &str) -> GtkBox {
    let body = GtkBox::new(Orientation::Vertical, 15);
    body.add_css_class("settings-body");
    body.append(&text(title, "settings-heading"));
    body.append(&text(description, "settings-subtitle"));
    body
}

fn refresh_theme(root: &GtkBox, header: &HeaderBar, prefs: &Preferences) {
    // GTK process only: do not modify the user's host desktop globally.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property("gtk-enable-animations", prefs.animations && !prefs.reduce_motion);
    }
    if prefs.high_contrast {
        root.add_css_class("high-contrast");
        header.add_css_class("high-contrast");
    } else {
        root.remove_css_class("high-contrast");
        header.remove_css_class("high-contrast");
    }
    if prefs.aero_glass {
        root.remove_css_class("aero-off");
        header.remove_css_class("aero-off");
    } else {
        root.add_css_class("aero-off");
        header.add_css_class("aero-off");
    }
    if prefs.dark_mode {
        root.add_css_class("settings-dark");
        header.add_css_class("settings-dark");
    } else {
        root.remove_css_class("settings-dark");
        header.remove_css_class("settings-dark");
    }
    for band in ["glass-low", "glass-medium", "glass-high", "glass-max"] {
        root.remove_css_class(band);
    }
    if prefs.aero_glass {
        let band = match prefs.blur_strength {
            0..=24 => "glass-low",
            25..=49 => "glass-medium",
            50..=79 => "glass-high",
            _ => "glass-max",
        };
        root.add_css_class(band);
    }
}

fn build_ui(app: &Application) {
    setup_melania_icon_theme();
    let css = gtk::CssProvider::new();
    #[allow(deprecated)]
    css.load_from_data(include_str!("../assets/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let window = ApplicationWindow::builder().application(app).title("Melania — Impostazioni")
        .default_width(870).default_height(600).build();
    window.add_css_class("melania-settings-window");
    // Native client-side decorations retain window dragging and window controls.
    let titlebar = HeaderBar::new();
    titlebar.add_css_class("settings-titlebar");
    titlebar.set_show_title_buttons(true);
    titlebar.set_decoration_layout(Some(":minimize,maximize,close"));
    let title_text = Label::new(Some("Melania — Impostazioni"));
    title_text.add_css_class("title");
    titlebar.set_title_widget(Some(&title_text));
    window.set_titlebar(Some(&titlebar));
    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.add_css_class("settings-root");
    let sidebar = GtkBox::new(Orientation::Vertical, 9);
    sidebar.add_css_class("settings-sidebar");
    sidebar.set_size_request(225, -1);
    let brand = Image::from_icon_name("melania");
    brand.set_pixel_size(70);
    brand.set_halign(gtk::Align::Start);
    sidebar.append(&brand);
    sidebar.append(&text("MELANIA", "settings-heading"));
    sidebar.append(&text("Impostazioni", "settings-subtitle"));
    // GNOME/COSMIC-style section search with familiar Cinnamon navigation.
    let category_search = Entry::new();
    category_search.set_placeholder_text(Some("Cerca nelle impostazioni…"));
    category_search.set_tooltip_text(Some("Filtra le sezioni delle Impostazioni"));
    category_search.add_css_class("settings-search");
    sidebar.append(&category_search);
    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let prefs = Rc::new(RefCell::new(melania_config::load().unwrap_or_default()));
    refresh_theme(&root, &titlebar, &prefs.borrow());
    let themed_root = root.clone();
    let themed_header = titlebar.clone();
    let apply_style: Rc<dyn Fn(&Preferences)> = Rc::new(move |p| {
        refresh_theme(&themed_root, &themed_header, p);
    });
    let appearance = page("Aspetto", "Personalizza l'identità visiva del desktop.");
    appearance.append(&saved_option("Aero Glass", "Trasparenze, riflessi e bordi Aero. La sfocatura reale richiede il compositore.", prefs.borrow().aero_glass, prefs.clone(), |p,v| p.aero_glass = v, Some(apply_style.clone())));
    appearance.append(&saved_option("Animazioni GTK", "Attiva le transizioni nelle finestre Melania, salvo la preferenza Riduci movimento.", prefs.borrow().animations, prefs.clone(), |p,v| p.animations = v, Some(apply_style.clone())));
    appearance.append(&saved_option("Tema scuro", "Applica subito la variante scura alle Impostazioni.", prefs.borrow().dark_mode, prefs.clone(), |p,v| p.dark_mode = v, Some(apply_style.clone())));
    appearance.append(&text("Intensità effetto vetro", "settings-hero"));
    let blur = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    blur.set_value(prefs.borrow().blur_strength as f64);
    blur.add_css_class("settings-intensity");
    blur.set_tooltip_text(Some("Regola la trasparenza del tema; il compositore userà in futuro il medesimo valore per la sfocatura."));
    appearance.append(&blur);
    let save_blur = Button::with_label("Salva intensità");
    let slider = blur.clone();
    let preferences = prefs.clone();
    let update_blur_style = apply_style.clone();
    save_blur.add_css_class("settings-save");
    save_blur.connect_clicked(move |_| {
        let mut p = preferences.borrow_mut();
        p.blur_strength = slider.value().round().clamp(0.0,100.0) as u8;
        if let Err(e) = melania_config::save(&p) { eprintln!("Melania Settings: {e}"); }
        update_blur_style(&p);
    });
    appearance.append(&save_blur);
    stack.add_titled(&appearance, Some("appearance"), "Aspetto");
    let desktop = page("Desktop e finestre", "Le preferenze avanzate previste per Melania.");
    desktop.append(&saved_option("Snap finestre", "Aggancio ai bordi gestito dal compositore dal prossimo avvio della sessione.", prefs.borrow().snap_windows, prefs.clone(), |p,v| p.snap_windows = v, None));
    desktop.append(&saved_option("Anteprime sulla barra", "Preferenza per le miniature delle applicazioni (ancora in sviluppo).", prefs.borrow().preview_windows, prefs.clone(), |p,v| p.preview_windows = v, None));
    desktop.append(&saved_option("Raggruppa finestre nella barra", "Un pulsante per applicazione con menu delle sue finestre. Richiede il riavvio del pannello Melania.", prefs.borrow().group_windows, prefs.clone(), |p,v| p.group_windows = v, None));
    desktop.append(&text("Lo Snap salvato viene letto dal compositore al successivo avvio. Le anteprime sulla barra restano in sviluppo.", "settings-subtitle"));
    stack.add_titled(&desktop, Some("desktop"), "Desktop");
    // Saved workspace count is applied on compositor/panel startup.
    // Changing mappings while windows are open would risk losing them.
    let workspaces = page("Spazi di lavoro", "Organizza finestre e applicazioni su desktop separati.");
    workspaces.append(&text("Numero di spazi di lavoro fissi (da 1 a 8)", "settings-hero"));
    let workspace_picker = SpinButton::with_range(1.0, 8.0, 1.0);
    workspace_picker.set_value(prefs.borrow().workspace_count as f64);
    workspace_picker.set_numeric(true);
    workspace_picker.set_halign(gtk::Align::Start);
    workspace_picker.add_css_class("settings-workspace-picker");
    workspaces.append(&workspace_picker);
    workspaces.append(&text(
        "Il nuovo numero si applica dopo il riavvio della sessione Melania, senza spostare le finestre già aperte.",
        "settings-subtitle"
    ));
    let apply_workspaces = Button::with_label("Salva numero di spazi di lavoro");
    apply_workspaces.add_css_class("settings-save");
    apply_workspaces.set_halign(gtk::Align::Start);
    let workspace_feedback = text("", "settings-subtitle");
    let count_control = workspace_picker.clone();
    let workspace_preferences = prefs.clone();
    let feedback = workspace_feedback.clone();
    apply_workspaces.connect_clicked(move |_| {
        let count = count_control.value_as_int().clamp(1, 8) as u8;
        let mut preferences = workspace_preferences.borrow_mut();
        preferences.workspace_count = count;
        match melania_config::save(&preferences) {
            Ok(()) => feedback.set_text(&format!(
                "{count} spazi di lavoro salvati. Riavvia la sessione Melania per applicarli."
            )),
            Err(e) => feedback.set_text(&format!("Impossibile salvare: {e}")),
        }
    });
    workspaces.append(&apply_workspaces);
    workspaces.append(&workspace_feedback);
    stack.add_titled(&workspaces, Some("workspaces"), "Spazi di lavoro");
    let network = page("Rete e Bluetooth", "I dispositivi si controllano dal Centro rapido della shell.");
    network.append(&text("Wi-Fi: NetworkManager (nmcli) · Bluetooth: BlueZ (bluetoothctl). Per poter agire, i servizi devono essere installati e l'utente deve avere i permessi richiesti.", "settings-subtitle"));
    stack.add_titled(&network, Some("network"), "Rete");
    let accessibility = page("Accessibilità", "Controlli leggeri e leggibili per le interfacce Melania.");
    accessibility.append(&saved_option(
        "Riduci movimento",
        "Disattiva le animazioni GTK di Melania; non cambia quelle delle altre applicazioni Linux.",
        prefs.borrow().reduce_motion, prefs.clone(),
        |p,v| p.reduce_motion = v, Some(apply_style.clone())
    ));
    accessibility.append(&saved_option(
        "Contrasto elevato",
        "Aumenta il contrasto di Settings e dei pannelli Aero; il compositor e le altre applicazioni non sono inclusi.",
        prefs.borrow().high_contrast, prefs.clone(),
        |p,v| p.high_contrast = v, Some(apply_style.clone())
    ));
    accessibility.append(&text("Le preferenze dei pannelli si applicano al prossimo avvio; questa finestra cambia subito.", "settings-subtitle"));
    stack.add_titled(&accessibility, Some("accessibility"), "Accessibilità");
    let system = page("Sistema", "Informazioni sull'ambiente Melania.");
    system.append(&text("Melania Desktop — pre-alpha", "settings-hero"));
    system.append(&text("Rust · GTK 4 · Wayland (compositore sperimentale in preparazione)", "settings-subtitle"));
    system.append(&text("Preferenze locali: $XDG_CONFIG_HOME/melania/settings.conf. Le impostazioni hardware sono delegate ai servizi Linux.", "settings-subtitle"));
    stack.add_titled(&system, Some("system"), "Sistema");
    let navigation: Rc<RefCell<Vec<Button>>> = Rc::new(RefCell::new(Vec::new()));
    let navigation_labels: Rc<RefCell<Vec<(Button, String)>>> = Rc::new(RefCell::new(Vec::new()));
    for (name, target) in [
        ("◉  Aspetto", "appearance"),
        ("▣  Desktop e finestre", "desktop"),
        ("▦  Spazi di lavoro", "workspaces"),
        ("⌁  Rete e Bluetooth", "network"),
        ("♿  Accessibilità", "accessibility"),
        ("⚙  Sistema", "system"),
    ] {
        let b = Button::with_label(name);
        let s = stack.clone();
        let nav = navigation.clone();
        b.connect_clicked(move |clicked| {
            s.set_visible_child_name(target);
            for item in nav.borrow().iter() {
                item.remove_css_class("active-section");
            }
            clicked.add_css_class("active-section");
        });
        if target == "appearance" { b.add_css_class("active-section"); }
        navigation.borrow_mut().push(b.clone());
        navigation_labels.borrow_mut().push((b.clone(), name.to_lowercase()));
        sidebar.append(&b);
    }
    let categories = navigation_labels.clone();
    category_search.connect_changed(move |search| {
        let query = search.text().to_lowercase();
        for (button, label) in categories.borrow().iter() {
            button.set_visible(query.is_empty() || label.contains(query.as_str()));
        }
    });
    let warning = text("Il tema Aero è disponibile nell'interfaccia. La sfocatura reale dello sfondo arriverà con il renderer Smithay.", "settings-warning");
    sidebar.append(&warning);
    root.append(&sidebar);
    let main_panel = GtkBox::new(Orientation::Vertical, 0);
    main_panel.add_css_class("settings-main");
    main_panel.set_hexpand(true);
    main_panel.set_vexpand(true);
    let scroll = gtk::ScrolledWindow::new();
    scroll.add_css_class("settings-scroll");
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&stack));
    main_panel.append(&scroll);
    root.append(&main_panel);
    window.set_child(Some(&root));
    window.present();
}


/// Prefer Melania's freedesktop icon theme inside Melania applications.
/// During development, assets can be loaded directly from the source tree;
/// packaged installations use XDG_DATA_HOME/icons/Melania instead.
/// Does not change the icon theme of the user's other desktop sessions.
fn setup_melania_icon_theme() {
    if let Some(display) = gtk::gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);
        let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../branding");
        if bundled.join("Melania-Aero-Classic/index.theme").is_file() {
            theme.add_search_path(&bundled);
        }
    }
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property("gtk-icon-theme-name", "Melania-Aero-Classic");
    }
}

fn main() -> gtk::glib::ExitCode {
    let app = Application::builder().application_id("org.melaniadesktop.Settings").build();
    app.connect_activate(build_ui);
    app.run()
}
