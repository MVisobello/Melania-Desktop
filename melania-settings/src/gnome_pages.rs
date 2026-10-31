//! GNOME-inspired, honest Melania Settings information architecture.
//! Settings displayed here never mutate host GNOME/Cinnamon configuration.
use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Image, Label, Orientation, Stack, Scale};
use crate::{symbols, sensors};
use std::{env, fs, process::Command};

fn label(value: &str, class: &str) -> Label {
    let label = Label::new(Some(value));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}
fn page(title: &str, subtitle: &str, icon: &str) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 7);
    page.add_css_class("settings-body");
    let heading = GtkBox::new(Orientation::Horizontal, 11);
    heading.add_css_class("settings-page-heading");
    let image = symbols::icon(icon, 32);
    image.add_css_class("settings-page-icon");
    heading.append(&image);
    let words = GtkBox::new(Orientation::Vertical, 2);
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
    let row = GtkBox::new(Orientation::Horizontal, 11);
    row.add_css_class("settings-info-row");
    row.set_valign(Align::Center);
    let image = symbols::icon(icon, 20);
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
    let layout = GtkBox::new(Orientation::Horizontal, 11);
    layout.set_valign(Align::Center);
    let image = symbols::icon(icon, 21);
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

fn battery_status() -> String {
    // Do not assume BAT0: convertible laptops and external battery packs use
    // BAT1, BAT2 or other BAT* devices.
    let batteries = fs::read_dir("/sys/class/power_supply").ok()
        .into_iter().flatten().flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("BAT"))
        .collect::<Vec<_>>();
    if batteries.is_empty() {
        return "Nessuna batteria rilevata (possibile computer fisso).".into();
    }
    batteries.iter().map(|battery| {
        let path = battery.path();
        let name = battery.file_name().to_string_lossy().to_string();
        let capacity = fs::read_to_string(path.join("capacity")).ok()
            .map(|x| format!("{}%", x.trim())).unwrap_or_else(|| "carica non disponibile".into());
        let status = fs::read_to_string(path.join("status")).ok()
            .map(|x| x.trim().to_string()).unwrap_or_else(|| "stato sconosciuto".into());
        format!("{name}: {capacity} · {status}")
    }).collect::<Vec<_>>().join("; ")
}
pub fn network_status() -> String {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device", "status"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let entries: Vec<_> = text.lines()
                .filter(|line| !line.trim().is_empty())
                .take(5).collect();
            if entries.is_empty() { "Nessuna interfaccia di rete rilevata.".into() }
            else { entries.join("  ·  ") }
        }
        _ => "NetworkManager non disponibile in questa sessione.".into(),
    }
}
fn root_storage() -> String {
    let output = Command::new("df").args(["-h", "--output=size,used,avail,pcent", "/"])
        .output();
    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines()
                .filter(|line| !line.trim().is_empty()).last() {
                let values: Vec<_> = line.split_whitespace().collect();
                if values.len() == 4 {
                    return format!("Totale {} · utilizzati {} · disponibili {} · occupazione {}",
                        values[0], values[1], values[2], values[3]);
                }
            }
        }
    }
    "Spazio disponibile non determinabile.".into()
}
fn parse_volume(output: &str) -> Option<f64> {
    let normalized = output.trim().strip_prefix("Volume:")?;
    let volume = normalized.split_whitespace().next()?.parse::<f64>().ok()?;
    if volume.is_finite() { Some((volume * 100.0).clamp(0.0, 100.0)) } else { None }
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
    let hardware = GtkBox::new(Orientation::Vertical, 0);
    hardware.add_css_class("settings-group-card");
    system.append(&hardware);
    for (target, icon, title, subtitle) in [
        ("display", "video-display", "Schermo", "Monitor, risoluzione e disposizione"),
        ("audio", "audio-card", "Audio", "Uscita, ingresso e dispositivi sonori"),
        ("power", "battery", "Alimentazione", "Batteria e risparmio energetico"),
        ("storage", "drive-harddisk", "Archiviazione", "Dischi e utilizzo dello spazio"),
    ] {
        hardware.append(&linked_row(stack, target, icon, title, subtitle));
    }
    group(&system, "Software e utenti");
    let software = GtkBox::new(Orientation::Vertical, 0);
    software.add_css_class("settings-group-card");
    system.append(&software);
    for (target, icon, title, subtitle) in [
        ("account", "avatar-default", "Utenti", "Informazioni sull'account corrente"),
        ("applications", "applications-other", "Applicazioni", "Applicazioni e associazioni file"),
        ("about", "dialog-information", "Informazioni sul sistema", "Versione di Melania e sistema operativo"),
    ] {
        software.append(&linked_row(stack, target, icon, title, subtitle));
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
    group(&display, "Rotazione dello schermo");
    display.append(&info_row("video-display", "Sensore di orientamento",
        sensors::explanation(sensors::accelerometer())));
    display.append(&info_row("dialog-information", "Rotazione manuale e blocco",
        "Non ancora disponibili: occorre una connessione al compositore Wayland, con sincronizzazione di touchscreen, finestre e area utile. Nessun comando esterno modifica lo schermo."));
    stack.add_titled(&display, Some("display"), "Schermo");

    let audio = page("Audio", "Volume e uscita audio tramite PipeWire/WirePlumber.", "audio-card");
    group(&audio, "Uscita audio");
    let current_volume = read_command("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .and_then(|output| parse_volume(&output));
    if let Some(volume) = current_volume {
        audio.append(&info_row("audio-volume-high", "Volume attuale",
            &format!("{volume:.0}% · dispositivo di uscita predefinito")));
        let controls = GtkBox::new(Orientation::Horizontal, 12);
        controls.add_css_class("settings-action-row");
        let level = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        level.set_value(volume);
        level.set_hexpand(true);
        level.set_tooltip_text(Some("Volume audio in percentuale"));
        let apply = Button::with_label("Applica volume");
        apply.add_css_class("settings-save");
        let feedback = label("", "settings-action-feedback");
        let scale = level.clone();
        let response = feedback.clone();
        apply.connect_clicked(move |_| {
            let requested = format!("{:.2}", scale.value().clamp(0.0, 100.0) / 100.0);
            let status = Command::new("wpctl")
                .args(["set-volume", "@DEFAULT_AUDIO_SINK@", requested.as_str()])
                .status();
            match status {
                Ok(status) if status.success() =>
                    response.set_text(&format!("Volume impostato: {:.0}%", scale.value())),
                _ => response.set_text("Impossibile cambiare il volume. Controlla WirePlumber."),
            }
        });
        controls.append(&level);
        controls.append(&apply);
        audio.append(&controls);
        audio.append(&feedback);
    } else {
        audio.append(&info_row("dialog-information", "Audio non disponibile",
            "Nessun dispositivo PipeWire/WirePlumber accessibile. Il controllo del volume non viene mostrato."));
    }
    stack.add_titled(&audio, Some("audio"), "Audio");

    let power = page("Alimentazione", "Stato e risparmio energetico.", "battery");
    group(&power, "Batteria");
    let charge = battery_status();
    power.append(&info_row("battery", "Stato dell'alimentazione", &charge));
    power.append(&info_row("dialog-information", "Risparmio energetico",
        "I profili energetici sono gestiti dai servizi Linux; Melania non li modifica da questa pagina."));
    stack.add_titled(&power, Some("power"), "Alimentazione");

    let storage = page("Archiviazione", "Informazioni sulle unità locali.", "drive-harddisk");
    group(&storage, "Dischi");
    let space = root_storage();
    storage.append(&info_row("drive-harddisk", "Filesystem principale", &space));
    storage.append(&info_row("dialog-information", "Gestione delle unità",
        "Le operazioni sui dischi rimangono affidate alle utility del sistema operativo."));
    stack.add_titled(&storage, Some("storage"), "Archiviazione");

    let bluetooth = page("Bluetooth", "Connessioni e periferiche Bluetooth.", "bluetooth");
    group(&bluetooth, "Stato");
    bluetooth.append(&info_row("bluetooth", "Controller Bluetooth",
        &read_command("bluetoothctl", &["show"]).unwrap_or_else(|| "Controller non disponibile o servizio BlueZ non attivo.".into())));
    stack.add_titled(&bluetooth, Some("bluetooth"), "Bluetooth");
    // The network overview is read-only until Melania integrates NetworkManager
    // controls; never misrepresent a setting as applied to the host.


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
    group(&privacy, "Cronologia locale");
    privacy.append(&info_row("document-open-recent", "App recenti nel menu Start",
        "Melania conserva localmente soltanto gli ID delle ultime applicazioni avviate dal suo menu Start, non i documenti o le attività di navigazione."));
    let erase_recent=Button::with_label("Cancella app recenti");
    erase_recent.add_css_class("settings-save");
    erase_recent.set_halign(Align::Start);
    let erase_feedback=label("", "settings-action-feedback");
    let message=erase_feedback.clone();
    erase_recent.connect_clicked(move |_| {
        match melania_config::recent_apps_path() {
            Some(path)=>match fs::remove_file(&path) {
                Ok(())=>message.set_text("Cronologia delle app recenti cancellata."),
                Err(error) if error.kind()==std::io::ErrorKind::NotFound=>
                    message.set_text("Nessuna cronologia di app recenti da cancellare."),
                Err(error)=>message.set_text(&format!("Impossibile cancellare la cronologia: {error}")),
            },
            None=>message.set_text("Directory dello stato utente non disponibile."),
        }
    });
    privacy.append(&erase_recent);
    privacy.append(&erase_feedback);
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

#[cfg(test)]
mod tests {
    use super::parse_volume;

    #[test]
    fn reads_pipewire_volume_without_exceeding_slider_range() {
        assert_eq!(parse_volume("Volume: 0.42"), Some(42.0));
        assert_eq!(parse_volume("Volume: 1.25 [MUTED]"), Some(100.0));
        assert_eq!(parse_volume("Volume: 0.00"), Some(0.0));
        assert_eq!(parse_volume("No audio backend"), None);
        assert_eq!(parse_volume("Volume: NaN"), None);
    }
}
