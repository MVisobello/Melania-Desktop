//! Settings design prototype. System changes are intentionally not wired yet.
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Image, Label, Orientation, Scale, Stack, Switch};
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
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 18);
    row.add_css_class("settings-card");
    let v = GtkBox::new(Orientation::Vertical, 5);
    v.set_hexpand(true);
    v.append(&text(title, "settings-hero"));
    v.append(&text(description, "settings-subtitle"));
    row.append(&v);
    let toggle = Switch::builder().active(current).build();
    toggle.connect_active_notify(move |sw| {
        let mut preferences = prefs.borrow_mut();
        change(&mut preferences, sw.is_active());
        if let Err(e) = melania_config::save(&preferences) {
            eprintln!("Melania Settings: impossibile salvare preferenze: {e}");
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
    let root = GtkBox::new(Orientation::Horizontal, 0);
    let sidebar = GtkBox::new(Orientation::Vertical, 9);
    sidebar.add_css_class("settings-sidebar");
    sidebar.set_size_request(225, -1);
    let brand = Image::from_icon_name("melania");
    brand.set_pixel_size(70);
    brand.set_halign(gtk::Align::Start);
    sidebar.append(&brand);
    sidebar.append(&text("MELANIA", "settings-heading"));
    sidebar.append(&text("Impostazioni", "settings-subtitle"));
    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let prefs = Rc::new(RefCell::new(melania_config::load().unwrap_or_default()));
    let appearance = page("Aspetto", "Personalizza l'identità visiva del desktop.");
    appearance.append(&saved_option("Aero Glass", "Preferenza grafica (rendering compositor non ancora implementato).", prefs.borrow().aero_glass, prefs.clone(), |p,v| p.aero_glass = v));
    appearance.append(&saved_option("Animazioni", "Memorizza l'uso delle animazioni.", prefs.borrow().animations, prefs.clone(), |p,v| p.animations = v));
    appearance.append(&saved_option("Tema scuro", "Aspetto predefinito per la shell Melania.", prefs.borrow().dark_mode, prefs.clone(), |p,v| p.dark_mode = v));
    appearance.append(&text("Intensità effetti visivi (salvata in preferenze)", "settings-hero"));
    let blur = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    blur.set_value(prefs.borrow().blur_strength as f64);
    appearance.append(&blur);
    let save_blur = Button::with_label("Salva intensità");
    let slider = blur.clone();
    let preferences = prefs.clone();
    save_blur.connect_clicked(move |_| {
        let mut p = preferences.borrow_mut();
        p.blur_strength = slider.value().round().clamp(0.0,100.0) as u8;
        if let Err(e) = melania_config::save(&p) { eprintln!("Melania Settings: {e}"); }
    });
    appearance.append(&save_blur);
    stack.add_titled(&appearance, Some("appearance"), "Aspetto");
    let desktop = page("Desktop e finestre", "Le preferenze avanzate previste per Melania.");
    desktop.append(&saved_option("Snap finestre", "Preferenza per l'aggancio ai bordi; integrazione futura con il compositore.", prefs.borrow().snap_windows, prefs.clone(), |p,v| p.snap_windows = v));
    desktop.append(&saved_option("Anteprime sulla barra", "Preferenza per le miniature delle applicazioni.", prefs.borrow().preview_windows, prefs.clone(), |p,v| p.preview_windows = v));
    desktop.append(&text("Spazi di lavoro: il modello Rust supporta workspace multipli; il rendering è ancora da collegare.", "settings-subtitle"));
    stack.add_titled(&desktop, Some("desktop"), "Desktop");
    let network = page("Rete e Bluetooth", "I dispositivi si controllano dal Centro rapido della shell.");
    network.append(&text("Wi-Fi: NetworkManager (nmcli) · Bluetooth: BlueZ (bluetoothctl). Per poter agire, i servizi devono essere installati e l'utente deve avere i permessi richiesti.", "settings-subtitle"));
    stack.add_titled(&network, Some("network"), "Rete");
    let system = page("Sistema", "Informazioni sull'ambiente Melania.");
    system.append(&text("Melania Desktop 0.3 — pre-alpha", "settings-hero"));
    system.append(&text("Rust · GTK 4 · Wayland (compositore sperimentale in preparazione)", "settings-subtitle"));
    system.append(&text("Preferenze locali: $XDG_CONFIG_HOME/melania/settings.conf. Le impostazioni hardware sono delegate ai servizi Linux.", "settings-subtitle"));
    stack.add_titled(&system, Some("system"), "Sistema");
    for (name, target) in [
        ("◉  Aspetto", "appearance"),
        ("▣  Desktop e finestre", "desktop"),
        ("⌁  Rete e Bluetooth", "network"),
        ("⚙  Sistema", "system"),
    ] {
        let b = Button::with_label(name);
        let s = stack.clone();
        b.connect_clicked(move |_| s.set_visible_child_name(target));
        sidebar.append(&b);
    }
    let warning = text("Le preferenze sono salvate; alcune funzioni grafiche saranno applicate nelle milestone successive.", "settings-warning");
    sidebar.append(&warning);
    root.append(&sidebar);
    root.append(&stack);
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
