//! GNOME-inspired, honest Melania Settings information architecture.
//! Settings displayed here never mutate host GNOME/Cinnamon configuration.
use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Image, Label, Orientation, Stack};
use std::{env, fs, process::Command};

fn label(value: &str, class: &str) -> Label {
    let label = Label::new(Some(value));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}
fn page(title: &str, subtitle: &str, icon: &str) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 14);
    page.add_css_class("settings-body");
    let heading = GtkBox::new(Orientation::Horizontal, 18);
    heading.add_css_class("settings-page-heading");
    let image = Image::from_icon_name(icon);
    image.set_pixel_size(42);
    image.add_css_class("settings-page-icon");
    heading.append(&image);
    let words = GtkBox::new(Orientation::Vertical, 4);
    words.append(&label(title, "settings-heading"));
    words.append(&label(subtitle, "settings-subtitle"));
    heading.append(&words);
    page.append(&heading);
    page
}
fn group(page: &GtkBox, title: &str) {
    let heading = label(title, "settings-group-label");
    page.append(&heading);
}
pub fn info_row(icon: &str, title: &str, description: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 15);
    row.add_css_class("settings-info-row");
    row.set_valign(Align::Center);
    let image = Image::from_icon_name(icon);
    image.set_pixel_size(23);
    image.add_css_class("settings-row-icon");
    row.append(&image);
    let words = GtkBox::new(Orientation::Vertical, 4);
    words.set_hexpand(true);
    words.append(&label(title, "settings-row-title"));
    words.append(&label(description, "settings-row-description"));
    row.append(&words);
    row
}
pub fn linked_row(stack: &Stack, target: &str, icon: &str, title: &str, description: &str) -> Button {
    let button = Button::new();
    button.add_css_class("settings-linked-row");
    button.set_hexpand(true);
    let layout = GtkBox::new(Orientation::Horizontal, 15);
    layout.set_valign(Align::Center);
    let image = Image::from_icon_name(icon);
    image.set_pixel_size(25);
    image.add_css_class("settings-row-icon");
    layout.append(&image);
    let words = GtkBox::new(Orientation::Vertical, 4);
    words.set_hexpand(true);
    words.append(&label(title, "settings-row-title"));
    words.append(&label(description, "settings-row-description"));
    layout.append(&words);
    let arrow = Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.add_css_class("settings-chevron");
    layout.append(&arrow);
    button.set_child(Some(&layout));
    let target = target.to_string();
    let stack = stack.clone();
    button.connect_clicked(move |_| stack.set_visible_child_name(&target));
    button
}
fn read_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() { return None; }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().map(str::trim).find(|s| !s.is_empty())?;
    Some(line.chars().take(170).collect())
}
fn os_name() -> String {
    fs::read_to_string("/etc/os-release").ok()
        .and_then(|text| text.lines().find_map(|line| {
            line.strip_prefix("PRETTY_NAME=")
                .map(|value| value.trim_matches('"').to_owned())
        }))
        .unwrap_or_else(|| env::consts::OS.to_string())
}

pub fn overview(stack: &Stack) -> GtkBox {
    let system = page("Sistema",
        "Gestisci le opzioni del sistema e i dispositivi del tuo computer.",
        "computer");
    group(&system, "Hardware");
    for (target, icon, title, subtitle) in [
        ("display", "video-display", "Schermo", "Monitor, risoluzione e disposizione"),
        ("audio", "audio-card", "Audio", "Uscita, ingresso e dispositivi sonori"),
        ("power", "battery", "Alimentazione", "Batteria e risparmio energetico"),
        ("storage", "drive-harddisk", "Archiviazione", "Dischi e utilizzo dello spazio"),
    ] {
        system.append(&linked_row(stack, target, icon, title, subtitle));
    }
    group(&system, "Software e utenti");
    for (target, icon, title, subtitle) in [
        ("account", "avatar-default", "Utenti", "Informazioni sull'account corrente"),
        ("applications", "application-x-executable", "Applicazioni", "Applicazioni e associazioni file"),
        ("about", "dialog-information", "Informazioni sul sistema", "Versione di Melania e sistema operativo"),
    ] {
        system.append(&linked_row(stack, target, icon, title, subtitle));
    }
    system
}
pub fn add_pages(stack: &Stack) {
    let display = page("Schermo", "Informazioni sul display della sessione.", "video-display");
    let monitor_count = gtk::gdk::Display::default()
        .map(|d| d.monitors().n_items()).unwrap_or(0);
    group(&display, "Monitor");
    display.append(&info_row("video-display", "Schermi rilevati",
        &format!("{monitor_count} monitor rilevati da GTK nella sessione corrente.")));
    display.append(&info_row("dialog-information", "Risoluzione e scala",
        "La configurazione dei monitor richiede ancora un'interfaccia al compositore Melania. Nessuna modifica è applicata al sistema ospitante."));
    stack.add_titled(&display, Some("display"), "Schermo");

    let audio = page("Audio", "Stato dei dispositivi audio Linux.", "audio-card");
    group(&audio, "Dispositivi");
    audio.append(&info_row("audio-volume-high", "Server audio",
        &read_command("wpctl", &["status"])
            .unwrap_or_else(|| "PipeWire/WirePlumber non interrogabile da questa sessione.".into())));
    audio.append(&info_row("dialog-information", "Configurazione audio",
        "I controlli dei volumi saranno disponibili dopo l'integrazione PipeWire. Questa pagina è attualmente informativa."));
    stack.add_titled(&audio, Some("audio"), "Audio");

    let power = page("Alimentazione", "Stato e risparmio energetico.", "battery");
    group(&power, "Batteria");
    let charge = fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .ok().map(|x| format!("Livello di carica: {}%", x.trim()))
        .unwrap_or_else(|| "Nessuna batteria BAT0 rilevata o informazione non disponibile.".into());
    power.append(&info_row("battery", "Stato dell'alimentazione", &charge));
    power.append(&info_row("dialog-information", "Risparmio energetico",
        "I profili energetici sono gestiti dai servizi Linux; Melania non li modifica da questa pagina."));
    stack.add_titled(&power, Some("power"), "Alimentazione");

    let storage = page("Archiviazione", "Informazioni sulle unità locali.", "drive-harddisk");
    group(&storage, "Dischi");
    let space = read_command("df", &["-h", "/"])
        .unwrap_or_else(|| "Spazio disponibile non determinabile.".into());
    storage.append(&info_row("drive-harddisk", "Filesystem principale", &space));
    storage.append(&info_row("dialog-information", "Gestione delle unità",
        "Le operazioni sui dischi rimangono affidate alle utility del sistema operativo."));
    stack.add_titled(&storage, Some("storage"), "Archiviazione");

    let bluetooth = page("Bluetooth", "Connessioni e periferiche Bluetooth.", "bluetooth");
    group(&bluetooth, "Stato");
    bluetooth.append(&info_row("bluetooth", "Controller Bluetooth",
        &read_command("bluetoothctl", &["show"]).unwrap_or_else(|| "Controller non disponibile o servizio BlueZ non attivo.".into())));
    stack.add_titled(&bluetooth, Some("bluetooth"), "Bluetooth");

    let applications = page("App", "Programmi e preferenze applicative.", "application-x-executable");
    group(&applications, "Applicazioni");
    applications.append(&info_row("applications-other", "Applicazioni installate",
        "Le applicazioni disponibili compaiono nel menu Start di Melania."));
    applications.append(&info_row("dialog-information", "App predefinite",
        "Le associazioni dei file sono gestite tramite XDG e le impostazioni delle singole applicazioni."));
    stack.add_titled(&applications, Some("applications"), "App");

    let account = page("Account", "Informazioni sull'utente attuale.", "avatar-default");
    group(&account, "Utente");
    account.append(&info_row("avatar-default", "Account corrente",
        &env::var("USER").unwrap_or_else(|_| "Utente non disponibile".into())));
    account.append(&info_row("dialog-information", "Sicurezza account",
        "Password e autorizzazioni sono gestite dal sistema Linux; Melania non modifica le credenziali."));
    stack.add_titled(&account, Some("account"), "Account");

    let datetime = page("Data e ora", "Orologio, calendario e fuso orario.", "x-office-calendar");
    group(&datetime, "Sistema");
    datetime.append(&info_row("x-office-calendar", "Ora del sistema",
        &read_command("date", &["+%d/%m/%Y %H:%M %Z"])
            .unwrap_or_else(|| "Ora non disponibile.".into())));
    datetime.append(&info_row("dialog-information", "Sincronizzazione",
        "Data e fuso orario vengono regolati dai servizi del sistema operativo."));
    stack.add_titled(&datetime, Some("datetime"), "Data e ora");

    let devices = page("Dispositivi", "Periferiche collegate al computer.", "input-keyboard");
    group(&devices, "Hardware");
    devices.append(&info_row("input-keyboard", "Input e periferiche",
        "Tastiera, mouse e dispositivi USB vengono gestiti dal sottosistema Linux."));
    stack.add_titled(&devices, Some("devices"), "Dispositivi");

    let privacy = page("Privacy e sicurezza", "Protezione dei dati e permessi.", "security-high");
    group(&privacy, "Privacy");
    privacy.append(&info_row("security-high", "Permessi applicazioni",
        "Le autorizzazioni delle applicazioni sono amministrate dai servizi e dai portali di Linux."));
    privacy.append(&info_row("user-trash", "Cestino",
        "Il gestore file Melania utilizza il Cestino di GIO, evitando l'eliminazione definitiva non richiesta."));
    stack.add_titled(&privacy, Some("privacy"), "Privacy");

    let region = page("Lingua e regione", "Impostazioni internazionali.", "preferences-desktop-locale");
    group(&region, "Lingua");
    region.append(&info_row("preferences-desktop-locale", "Lingua della sessione",
        &env::var("LANG").unwrap_or_else(|_| "Locale non disponibile".into())));
    stack.add_titled(&region, Some("region"), "Lingua e regione");

    let about = page("Informazioni", "Informazioni su Melania Desktop.", "dialog-information");
    group(&about, "Versione");
    about.append(&info_row("computer", "Melania Desktop",
        &format!("Versione {} · progetto sperimentale GTK 4 / Wayland",
                 env!("CARGO_PKG_VERSION"))));
    about.append(&info_row("computer", "Sistema Linux",
        &format!("{} · architettura {}", os_name(), env::consts::ARCH)));
    about.append(&info_row("dialog-information", "Stato del progetto",
        "Alcune pagine sono informative. Le opzioni che possono essere cambiate sono contrassegnate da controlli reali."));
    stack.add_titled(&about, Some("about"), "Informazioni");
}
