//! Melania GTK4 Settings: GNOME information architecture with real local preferences.
mod gnome_pages;
mod connectivity_controls;
mod display_controls;
mod xfce_catalog;
mod xfce_defaults;
mod xfce_autostart;
mod xfce_pages;
mod xfce_panel;
mod xfce_desktop;
mod symbols;
mod sensors;
#[path = "../../branding/window_controls.rs"]
mod window_controls;
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
    v.append(&text(title, "settings-row-title"));
    v.append(&text(description, "settings-row-description"));
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

fn page(title: &str) -> GtkBox {
    let body = GtkBox::new(Orientation::Vertical, 12);
    body.add_css_class("settings-body");
    let introduction = GtkBox::new(Orientation::Vertical, 5);
    introduction.add_css_class("settings-introduction");
    introduction.append(&text(title, "settings-heading"));
    body.append(&introduction);
    body
}

fn section(title: &str) -> Label { text(title, "settings-section-heading") }

fn refresh_theme(root: &GtkBox, header: &HeaderBar, prefs: &Preferences) {
    // GTK process only: do not modify the user's host desktop globally.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property("gtk-enable-animations", prefs.animations && !prefs.reduce_motion);
        settings.set_property("gtk-application-prefer-dark-theme", prefs.dark_mode);
        settings.set_property("gtk-icon-theme-name", melania_config::resolved_icon_theme(prefs));
    }
    for name in ["melania-theme-light", "melania-theme-dark", "melania-theme-aero"] {
        root.remove_css_class(name);
        header.remove_css_class(name);
    }
    let theme_class = format!("melania-theme-{}", prefs.appearance_theme());
    root.add_css_class(&theme_class);
    header.add_css_class(&theme_class);
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

fn build_ui(
    app: &Application,
    initial_page: &str,
    shared_stack: &Rc<RefCell<Option<Stack>>>,
) {
    setup_melania_icon_theme();
    let css = gtk::CssProvider::new();
    #[allow(deprecated)]
    css.load_from_data(include_str!("../assets/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let rows_css = gtk::CssProvider::new();
    rows_css.load_from_data(include_str!("../assets/gnome-overview.css"));
    let chrome_css = gtk::CssProvider::new();
    chrome_css.load_from_data(include_str!("../../branding/window-chrome-win7.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &rows_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        gtk::style_context_add_provider_for_display(&display, &chrome_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
    }
    let window = ApplicationWindow::builder().application(app).title("Melania — Impostazioni")
        .default_width(1020).default_height(690).build();
    window.add_css_class("melania-settings-window");
    // Shared component: same Win7 buttons, glyphs and actions as Files.
    let titlebar = HeaderBar::new();
    titlebar.add_css_class("settings-titlebar");
    let title_text = Label::new(Some("Impostazioni"));
    title_text.add_css_class("title");
    titlebar.set_title_widget(Some(&title_text));
    // Responsive navigation keeps the Settings sidebar usable on narrow screens.
    let navigation_toggle = Button::with_label("☰");
    navigation_toggle.add_css_class("settings-sidebar-toggle");
    navigation_toggle.set_tooltip_text(Some("Mostra o nascondi il menu (Ctrl+B)"));
    titlebar.pack_start(&navigation_toggle);
    window_controls::install(&window, &titlebar);
    window.set_titlebar(Some(&titlebar));
    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.add_css_class("settings-root");
    let sidebar = GtkBox::new(Orientation::Vertical, 5);
    sidebar.add_css_class("settings-sidebar");
    sidebar.set_size_request(250, -1);
    sidebar.set_hexpand(false);
    let brand_row = GtkBox::new(Orientation::Horizontal, 10);
    brand_row.add_css_class("settings-brand");
    let brand = symbols::icon("melania", 29);
    brand_row.append(&brand);
    brand_row.append(&text("Melania", "settings-brand-name"));
    sidebar.append(&brand_row);
    let category_search = Entry::new();
    category_search.set_placeholder_text(Some("Cerca nelle impostazioni…"));
    category_search.set_width_chars(16);
    category_search.set_max_width_chars(22);
    category_search.set_tooltip_text(Some("Filtra le categorie"));
    category_search.add_css_class("settings-search");
    sidebar.append(&category_search);
    let navigation_scroll = gtk::ScrolledWindow::new();
    navigation_scroll.set_vexpand(true);
    navigation_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    navigation_scroll.add_css_class("settings-navigation-scroll");
    let navigation_list = GtkBox::new(Orientation::Vertical, 1);
    navigation_list.add_css_class("settings-navigation-list");
    navigation_scroll.set_child(Some(&navigation_list));
    sidebar.append(&navigation_scroll);
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
    let appearance = page("Aspetto");
    let appearance_row = GtkBox::new(Orientation::Horizontal, 14);
    appearance_row.add_css_class("settings-card");
    let appearance_summary = GtkBox::new(Orientation::Vertical, 3);
    appearance_summary.set_hexpand(true);
    appearance_summary.append(&text("Stile di Melania", "settings-row-title"));
    appearance_summary.append(&text("Light: chiaro e sobrio. Dark: scuro ispirato a GNOME. Aero: classico con riflessi.", "settings-row-description"));
    appearance_row.append(&appearance_summary);
    let appearance_picker = gtk::DropDown::from_strings(&["Light · Chiaro", "Dark · Scuro", "Aero · Classico"]);
    appearance_picker.add_css_class("settings-appearance-choice");
    let selected_theme = match prefs.borrow().appearance_theme() { "light" => 0, "dark" => 1, _ => 2 };
    appearance_picker.set_selected(selected_theme);
    let prefs_for_theme = prefs.clone();
    let apply_theme = apply_style.clone();
    appearance_picker.connect_selected_notify(move |picker| {
        let mode = match picker.selected() { 0 => "light", 1 => "dark", 2 => "aero", _ => return };
        let mut p = prefs_for_theme.borrow_mut();
        p.set_appearance_theme(mode);
        if p.icon_theme.starts_with("Melania-Fluent-") {
            p.icon_theme = if mode == "dark" { "Melania-Fluent-Dark" } else { "Melania-Fluent-Light" }.into();
        }
        if let Err(e) = melania_config::save(&p) {
            eprintln!("Melania Settings: tema non salvato: {e}");
            return;
        }
        apply_theme(&p);
    });
    appearance_row.append(&appearance_picker);
    appearance.append(&appearance_row);
    appearance.append(&section("Comportamento"));
    appearance.append(&saved_option("Animazioni GTK", "Attiva le transizioni nelle finestre Melania, salvo la preferenza Riduci movimento.", prefs.borrow().animations, prefs.clone(), |p,v| p.animations = v, Some(apply_style.clone())));
    appearance.append(&section("Pacchetto di icone"));
    let icon_variants = gtk::DropDown::from_strings(&[
        "Melania Fluent Light (predefinito)", "Melania Fluent Dark", "Melania Fluent Light on Dark Panel",
        "Melania Aero Light", "Melania Aero Dark", "Melania Aero Light on Dark Panel"
    ]);
    icon_variants.add_css_class("settings-icon-choice");
    let selected = melania_config::ICON_THEMES.iter()
        .position(|&name|name==prefs.borrow().icon_theme).unwrap_or(0);
    icon_variants.set_selected(selected as u32);
    icon_variants.set_tooltip_text(Some(
        "Scegli Melania Fluent (concept approvato) oppure i temi Aero precedenti. Le app applicano il tema al riavvio."
    ));
    let prefs_for_icons = prefs.clone();
    icon_variants.connect_selected_notify(move |dropdown| {
        let index=dropdown.selected() as usize;
        let Some(&variant)=melania_config::ICON_THEMES.get(index) else{return;};
        let mut preferences = prefs_for_icons.borrow_mut();
        preferences.icon_theme=variant.to_string();
        if let Err(e)=melania_config::save(&preferences) {
            eprintln!("Impossibile salvare il tema icone: {e}");
            return;
        }
        if let Some(settings)=gtk::Settings::default() {
            settings.set_property("gtk-icon-theme-name",
                melania_config::resolved_icon_theme(&preferences));
        }
    });
    // Keep the visible icon-variant selector synchronized when switching
    // Light/Dark/Aero, instead of leaving the old Fluent label on screen.
    let icon_dropdown = icon_variants.clone();
    let prefs_for_icon_sync = prefs.clone();
    appearance_picker.connect_selected_notify(move |_| {
        let index = {
            let p = prefs_for_icon_sync.borrow();
            melania_config::ICON_THEMES.iter()
                .position(|name| *name == p.icon_theme).unwrap_or(0) as u32
        };
        if icon_dropdown.selected() != index {
            icon_dropdown.set_selected(index);
        }
    });
    let icon_row = GtkBox::new(Orientation::Horizontal, 16);
    icon_row.add_css_class("settings-card");
    let icon_summary = GtkBox::new(Orientation::Vertical, 4);
    icon_summary.set_hexpand(true);
    icon_summary.append(&text("Tema icone", "settings-row-title"));
    icon_summary.append(&text("Fluent è predefinito; le app già aperte potrebbero richiedere un riavvio.", "settings-row-description"));
    icon_row.append(&icon_summary);
    icon_row.append(&icon_variants);
    appearance.append(&icon_row);
    appearance.append(&section("Effetti visivi"));
    appearance.append(&text("Intensità effetto vetro", "settings-row-title"));
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
    let desktop = page("Desktop e finestre");
    desktop.append(&saved_option("Snap finestre", "Aggancio ai bordi gestito dal compositore dal prossimo avvio della sessione.", prefs.borrow().snap_windows, prefs.clone(), |p,v| p.snap_windows = v, None));
    desktop.append(&saved_option("Anteprime sulla barra", "Catture GPU delle sole finestre della sessione Melania nel selettore al passaggio del mouse. Riavvia il pannello.", prefs.borrow().preview_windows, prefs.clone(), |p,v| p.preview_windows = v, None));
    desktop.append(&saved_option("Raggruppa finestre nella barra", "Un pulsante per applicazione con menu delle sue finestre. Richiede il riavvio del pannello Melania.", prefs.borrow().group_windows, prefs.clone(), |p,v| p.group_windows = v, None));
    stack.add_titled(&desktop, Some("desktop"), "Desktop");
    // Saved workspace count is applied on compositor/panel startup.
    // Changing mappings while windows are open would risk losing them.
    let workspaces = page("Spazi di lavoro");
    workspaces.append(&text("Numero di spazi di lavoro fissi (da 1 a 8)", "settings-hero"));
    let workspace_picker = SpinButton::with_range(1.0, 8.0, 1.0);
    workspace_picker.set_value(prefs.borrow().workspace_count as f64);
    workspace_picker.set_numeric(true);
    workspace_picker.set_halign(gtk::Align::Start);
    workspace_picker.add_css_class("settings-workspace-picker");
    workspaces.append(&workspace_picker);
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
    let network = page("Rete");
    connectivity_controls::append_wifi(&network);
    let live_network = text(&gnome_pages::network_status(), "settings-row-description");
    live_network.add_css_class("settings-card");
    network.append(&live_network);
    let refresh_network = Button::with_label("Aggiorna stato della rete");
    refresh_network.add_css_class("settings-save");
    refresh_network.add_css_class("settings-refresh-button");
    refresh_network.set_halign(gtk::Align::Start);
    let current_network = live_network.clone();
    refresh_network.connect_clicked(move |_| {
        current_network.set_text(&gnome_pages::network_status());
    });
    network.append(&refresh_network);
    stack.add_titled(&network, Some("network"), "Rete");
    let accessibility = page("Accessibilità");
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
    stack.add_titled(&accessibility, Some("accessibility"), "Accessibilità");
    let keyboard = page("Scorciatoie");
    keyboard.append(&text("Alt + Tab  ·  Finestra successiva", "settings-hero"));
    keyboard.append(&text("Alt + Maiusc + Tab  ·  Finestra precedente", "settings-subtitle"));
    keyboard.append(&text("Ctrl + Alt + Freccia sinistra/destra  ·  Cambia spazio di lavoro", "settings-hero"));
    keyboard.append(&text("Super + E  ·  Apri Melania Files", "settings-hero"));
    stack.add_titled(&keyboard, Some("shortcuts"), "Scorciatoie");
    // Summary and category pages reflect actual status, without fake hardware switches.
    gnome_pages::add_pages(&stack);
    xfce_pages::add_pages(&stack);
    let system = gnome_pages::overview(&stack);
    stack.add_titled(&system, Some("system"), "Sistema");
    let notifications = page("Notifiche");
    notifications.append(&saved_option("Non disturbare",
        "Silenzia gli avvisi di Melania finché non disattivi l'opzione.",
        prefs.borrow().do_not_disturb, prefs.clone(), |p,v| p.do_not_disturb=v, None));
    stack.add_titled(&notifications, Some("notifications"), "Notifiche");
    // Expose this live stack only to our own application's command handler,
    // so a second invocation can navigate the existing Settings window.
    *shared_stack.borrow_mut() = Some(stack.clone());
    if is_valid_page(initial_page) {
        stack.set_visible_child_name(xfce_pages::canonical_page(initial_page));
    }
    let navigation: Rc<RefCell<Vec<(Button, &'static str)>>> = Rc::new(RefCell::new(Vec::new()));
    let navigation_headings: Rc<RefCell<Vec<Label>>> = Rc::new(RefCell::new(Vec::new()));
    let navigation_labels: Rc<RefCell<Vec<(Button, String)>>> = Rc::new(RefCell::new(Vec::new()));
    // Every real Settings page appears in this sidebar; no extra catalogue
    // is required to discover a live control. The same registry validates CLI pages.
    for &(name, target, icon_name) in SETTINGS_NAVIGATION.iter() {
        // Group headings make the long navigation easier to scan.
        let group = match target {
            "system" => Some("SISTEMA"),
            "appearance" => Some("PERSONALIZZAZIONE"),
            "applications" => Some("APPLICAZIONI"),
            "account" => Some("ACCOUNT E PRIVACY"),
            "region" => Some("ALTRE IMPOSTAZIONI"),
            _ => None,
        };
        if let Some(group) = group {
            let heading = text(group, "settings-nav-group");
            navigation_headings.borrow_mut().push(heading.clone());
            navigation_list.append(&heading);
        }
        let button = Button::new();
        button.add_css_class("settings-nav-item");
        button.set_hexpand(true);
        button.set_tooltip_text(Some(name));
        let row = GtkBox::new(Orientation::Horizontal, 9);
        row.add_css_class("settings-nav-content");
        row.append(&symbols::icon(icon_name, 19));
        let caption = Label::new(Some(name));
        caption.set_xalign(0.0);
        caption.set_hexpand(true);
        caption.set_max_width_chars(25);
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&caption);
        button.set_child(Some(&row));
        let navigation_stack = stack.clone();
        button.connect_clicked(move |_| navigation_stack.set_visible_child_name(target));
        if target == xfce_pages::canonical_page(initial_page) { button.add_css_class("active-section"); }
        navigation.borrow_mut().push((button.clone(), target));
        navigation_labels.borrow_mut().push((button.clone(), name.to_lowercase()));
        navigation_list.append(&button);
    }
    // Selected state must use the actual target, not SETTINGS_PAGES ordering.
    let buttons = navigation.clone();
    stack.connect_visible_child_name_notify(move |stack| {
        let current = stack.visible_child_name();
        for (button, target) in buttons.borrow().iter() {
            if current.as_deref() == Some(*target) {
                button.add_css_class("active-section");
            } else {
                button.remove_css_class("active-section");
            }
        }
    });
    let categories = navigation_labels.clone();
    let group_labels = navigation_headings.clone();
    category_search.connect_changed(move |search| {
        let query = search.text().trim().to_lowercase();
        for heading in group_labels.borrow().iter() {
            heading.set_visible(query.is_empty());
        }
        for (button, label) in categories.borrow().iter() {
            button.set_visible(query.is_empty() || label.contains(query.as_str()));
        }
    });
    sidebar.append(&text("Melania Desktop  ·  Impostazioni locali", "settings-sidebar-footer"));
    let sidebar_revealer = gtk::Revealer::new();
    sidebar_revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    sidebar_revealer.set_transition_duration(170);
    sidebar_revealer.set_reveal_child(true);
    sidebar_revealer.set_child(Some(&sidebar));
    root.append(&sidebar_revealer);
    let side = sidebar_revealer.clone();
    navigation_toggle.connect_clicked(move |_| {
        side.set_reveal_child(!side.reveals_child());
    });
    // Ctrl+B toggles the sidebar without relying on tiny window buttons.
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_scope(gtk::ShortcutScope::Managed);
    let side_for_shortcut = sidebar_revealer.clone();
    let shortcut_action = gtk::CallbackAction::new(move |_, _| {
        side_for_shortcut.set_reveal_child(!side_for_shortcut.reveals_child());
        gtk::glib::Propagation::Stop
    });
    let trigger = gtk::ShortcutTrigger::parse_string("<Control>b")
        .expect("valid Ctrl+B accelerator");
    shortcuts.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(shortcut_action)));
    window.add_controller(shortcuts);
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

// Canonical category map: all 25 registered pages have an accessible sidebar
// entry, and the CLI uses the same names. Legacy XFCE deep links are redirected
// to these pages rather than opening redundant, unreachable placeholder tabs.
const SETTINGS_NAVIGATION: [(&str, &str, &str); 25] = [
    ("Sistema", "system", "computer"),
    ("Schermo", "display", "video-display"),
    ("Audio", "audio", "audio-volume-high"),
    ("Alimentazione", "power", "battery"),
    ("Archiviazione", "storage", "drive-harddisk"),
    ("Rete", "network", "network-workgroup"),
    ("Bluetooth", "bluetooth", "bluetooth"),
    ("Dispositivi", "devices", "input-keyboard"),
    ("Sessione e avvio", "xfce-session", "system-run"),
    ("Aspetto", "appearance", "preferences-desktop-theme"),
    ("Scrivania e sfondo", "xfce-desktop", "user-desktop"),
    ("Desktop e finestre", "desktop", "preferences-system-windows"),
    ("Pannello e barra", "xfce-panel", "preferences-desktop-toolbar"),
    ("Spazi di lavoro", "workspaces", "view-grid"),
    ("Scorciatoie", "shortcuts", "input-keyboard"),
    ("Notifiche", "notifications", "preferences-system-notifications"),
    ("Accessibilità", "accessibility", "preferences-desktop-accessibility"),
    ("App", "applications", "applications-other"),
    ("App predefinite", "xfce-preferred", "applications-default"),
    ("Associazioni file", "xfce-mime", "text-x-generic"),
    ("Account", "account", "avatar-default"),
    ("Privacy e sicurezza", "privacy", "security-high"),
    ("Lingua e regione", "region", "preferences-desktop-locale"),
    ("Data e ora", "datetime", "x-office-calendar"),
    ("Informazioni", "about", "dialog-information"),
];
fn is_valid_page(page: &str) -> bool {
    SETTINGS_NAVIGATION.iter().any(|(_, target, _)| *target == page)
        || xfce_pages::is_known_page(page)
}

fn main() -> gtk::glib::ExitCode {
    // Handle remote positional page requests even when Settings is already
    // open, using GTK's normal single-instance application activation.
    let app = Application::builder()
        .application_id("org.melaniadesktop.Settings")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let requested_page: Rc<RefCell<String>> = Rc::new(RefCell::new("system".into()));
    let stack: Rc<RefCell<Option<Stack>>> = Rc::new(RefCell::new(None));
    let requested = requested_page.clone();
    let stack_for_activate = stack.clone();
    app.connect_activate(move |app| {
        let page = requested.borrow().clone();
        if let Some(window) = app.active_window() {
            if let Some(ref stack) = *stack_for_activate.borrow() {
                stack.set_visible_child_name(&page);
            }
            window.present();
        } else {
            build_ui(app, &page, &stack_for_activate);
        }
    });
    let requested = requested_page.clone();
    app.connect_command_line(move |app, command_line| {
        let args = command_line.arguments();
        let page = args.get(1).and_then(|arg| arg.to_str()).unwrap_or("system");
        if !is_valid_page(page) {
            eprintln!("Melania Settings: sezione non valida: {page}");
            return gtk::glib::ExitCode::FAILURE;
        }
        *requested.borrow_mut() = xfce_pages::canonical_page(page).to_string();
        app.activate();
        gtk::glib::ExitCode::SUCCESS
    });
    app.run()
}
