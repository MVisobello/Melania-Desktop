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
fn page(title: &str, icon: &str) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 7);
    page.add_css_class("settings-body");
    let heading = GtkBox::new(Orientation::Horizontal, 11);
    heading.add_css_class("settings-page-heading");
    let image = symbols::icon(icon, 32);
    image.add_css_class("settings-page-icon");
    heading.append(&image);
    let words = GtkBox::new(Orientation::Vertical, 2);
    words.append(&label(title, "settings-heading"));
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
/// Available power-profiles-daemon modes, whitelisted before any call to
/// powerprofilesctl set. Models do not always provide performance mode.
fn parse_power_profiles(output:&str)->Vec<&'static str> {
    let mut result=Vec::new();
    for line in output.lines() {
        let trimmed=line.trim().trim_start_matches('*').trim();
        let Some(mode)=trimmed.strip_suffix(':') else {continue;};
        let value=match mode {
            "power-saver"=>Some("power-saver"),
            "balanced"=>Some("balanced"),
            "performance"=>Some("performance"),
            _=>None,
        };
        if let Some(value)=value {
            if !result.contains(&value){result.push(value);}
        }
    }
    result
}

/// Read a PipeWire node without changing the current mixer state.
fn audio_mute_label(target: &str, muted: bool) -> &'static str {
    if target=="@DEFAULT_AUDIO_SOURCE@" {
        if muted {"Attiva microfono"} else {"Disattiva microfono"}
    } else if muted {"Riattiva audio"} else {"Disattiva audio"}
}
fn audio_status(target: &str) -> Option<(f64,bool)> {
    let response=read_command("wpctl", &["get-volume",target])?;
    let volume=parse_volume(&response)?;
    Some((volume,response.contains("[MUTED]")))
}
fn refresh_audio_status(target: &str, slider: &Scale, mute: &Button,
                        reading: &Label) -> bool {
    let Some((volume,muted))=audio_status(target) else {return false;};
    slider.set_value(volume);
    mute.set_label(audio_mute_label(target,muted));
    reading.set_text(&format!("Volume attuale: {volume:.0}%{}",
        if muted {" · disattivato"} else {""}));
    true
}
fn audio_device_controls(page: &GtkBox, title: &str, target: &'static str) {
    group(page,title);
    let Some((volume,muted))=audio_status(target) else {
        page.append(&info_row("dialog-information",title,
            "Dispositivo PipeWire/WirePlumber non disponibile: nessun controllo fittizio."));
        return;
    };
    let level=Scale::with_range(Orientation::Horizontal,0.0,100.0,1.0);
    level.set_value(volume);
    level.set_hexpand(true);
    level.set_tooltip_text(Some("Volume del dispositivo in percentuale"));
    let apply=Button::with_label("Applica volume");
    apply.add_css_class("settings-save");
    let mute=Button::with_label(audio_mute_label(target,muted));
    mute.add_css_class("settings-save");
    let refresh=Button::with_label("Aggiorna");
    refresh.add_css_class("settings-save");
    let reading=label(&format!("Volume attuale: {volume:.0}%{}",
        if muted {" · disattivato"} else {""}),"settings-row-description");
    let feedback=label("","settings-action-feedback");
    let controls=GtkBox::new(Orientation::Horizontal,8);
    controls.add_css_class("settings-action-row");
    controls.append(&level);
    controls.append(&apply);
    controls.append(&mute);
    controls.append(&refresh);
    page.append(&reading);
    page.append(&controls);
    page.append(&feedback);

    let slider=level.clone();
    let mute_button=mute.clone();
    let state=reading.clone();
    let message=feedback.clone();
    apply.connect_clicked(move |_| {
        // Only an explicit click changes a system PipeWire volume. The target
        // is a compile-time selected sink/source, never a user command.
        let requested=format!("{:.2}",slider.value().clamp(0.0,100.0)/100.0);
        let result=Command::new("wpctl")
            .args(["set-volume",target,requested.as_str()]).status();
        if !result.is_ok_and(|status|status.success()) {
            message.set_text("Impossibile applicare il volume. Verifica WirePlumber.");
        } else if refresh_audio_status(target,&slider,&mute_button,&state) {
            message.set_text("Volume applicato.");
        } else {
            message.set_text("Comando eseguito, ma non è possibile verificare il volume.");
        }
    });
    let slider=level.clone();
    let mute_button=mute.clone();
    let state=reading.clone();
    let message=feedback.clone();
    mute.connect_clicked(move |_| {
        let result=Command::new("wpctl").args(["set-mute",target,"toggle"]).status();
        if !result.is_ok_and(|status|status.success()) {
            message.set_text("Impossibile modificare lo stato audio.");
        } else if refresh_audio_status(target,&slider,&mute_button,&state) {
            message.set_text("Stato audio aggiornato.");
        } else {
            message.set_text("Comando eseguito, ma il dispositivo non risponde.");
        }
    });
    let slider=level.clone();
    let mute_button=mute.clone();
    let state=reading.clone();
    let message=feedback.clone();
    refresh.connect_clicked(move |_| {
        if refresh_audio_status(target,&slider,&mute_button,&state) {
            message.set_text("");
        } else {
            message.set_text("Impossibile leggere il dispositivo audio.");
        }
    });
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
    let system = page("Sistema", "computer");
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
    let display = page("Schermo", "video-display");
    let monitor_count = gtk::gdk::Display::default()
        .map(|d| d.monitors().n_items()).unwrap_or(0);
    group(&display, "Monitor");
    display.append(&info_row("video-display", "Schermi rilevati",
        &format!("{monitor_count} monitor rilevati da GTK nella sessione corrente.")));
    display.append(&info_row("dialog-information", "Risoluzione e scala",
        "La configurazione dei monitor richiede ancora un'interfaccia al compositore Melania. Nessuna modifica è applicata al sistema ospitante."));
    group(&display, "Rotazione dello schermo");
    display.append(&info_row("video-display", "Sensore di orientamento",
        "Disponibilità del sensore interrogata tramite iio-sensor-proxy."));
    let sensor_status=label(sensors::explanation(sensors::accelerometer()),"settings-row-description");
    sensor_status.add_css_class("settings-card");
    display.append(&sensor_status);
    let sensor_refresh=Button::with_label("Rileva nuovamente i sensori");
    sensor_refresh.add_css_class("settings-save");
    sensor_refresh.set_halign(Align::Start);
    let sensor_description=sensor_status.clone();
    sensor_refresh.connect_clicked(move |_| {
        sensor_description.set_text(sensors::explanation(sensors::accelerometer()));
    });
    display.append(&sensor_refresh);
    display.append(&info_row("dialog-information", "Rotazione manuale e blocco",
        "Non ancora disponibili: occorre una connessione al compositore Wayland, con sincronizzazione di touchscreen, finestre e area utile. Nessun comando esterno modifica lo schermo."));
    stack.add_titled(&display, Some("display"), "Schermo");

    let audio = page("Audio", "audio-card");
    // Input and output are separate PipeWire nodes. A missing device gets a
    // clear unavailable state; no fake slider or host-wide mixer fallback.
    audio_device_controls(&audio, "Uscita audio", "@DEFAULT_AUDIO_SINK@");
    audio_device_controls(&audio, "Microfono", "@DEFAULT_AUDIO_SOURCE@");
    stack.add_titled(&audio, Some("audio"), "Audio");

    let power = page("Alimentazione", "battery");
    group(&power, "Batteria");
    let charge = battery_status();
    power.append(&info_row("battery", "Stato dell'alimentazione", &charge));
    group(&power,"Profili energetici");
    // XFCE-style power profile choice backed by the real Linux service. No
    // toggle is shown when power-profiles-daemon is absent or unsupported.
    let profile_list=Command::new("powerprofilesctl").arg("list").output().ok()
        .filter(|result|result.status.success())
        .map(|result|parse_power_profiles(&String::from_utf8_lossy(&result.stdout)))
        .unwrap_or_default();
    if profile_list.is_empty() {
        power.append(&info_row("dialog-information","Profili non disponibili",
            "power-profiles-daemon non è disponibile o non espone profili supportati."));
    } else {
        let active_profile=read_command("powerprofilesctl",&["get"]).unwrap_or_default();
        let labels=profile_list.iter().map(|name|match *name {
            "power-saver"=>"Risparmio energetico",
            "balanced"=>"Bilanciato",
            "performance"=>"Prestazioni elevate",
            _=>*name
        }).collect::<Vec<_>>();
        let selector=gtk::DropDown::from_strings(&labels);
        let selected=profile_list.iter().position(|name|*name==active_profile)
            .unwrap_or(0);
        selector.set_selected(selected as u32);
        selector.set_hexpand(true);
        let controls=GtkBox::new(Orientation::Horizontal,10);
        controls.add_css_class("settings-action-row");
        controls.append(&selector);
        let apply=Button::with_label("Applica profilo");
        apply.add_css_class("settings-save");
        controls.append(&apply);
        let feedback=label("La scelta modifica il profilo energetico Linux, anche nelle altre sessioni.",
            "settings-row-description");
        let response=feedback.clone();
        apply.connect_clicked(move |_|{
            let Some(&profile)=profile_list.get(selector.selected() as usize) else {return;};
            let result=Command::new("powerprofilesctl").args(["set",profile]).status();
            match result {
                Ok(status) if status.success() =>response.set_text(
                    &format!("Profilo energetico applicato: {profile}.")
                ),
                Ok(status)=>response.set_text(&format!(
                    "Impossibile applicare il profilo {profile}: codice {status}."
                )),
                Err(error)=>response.set_text(&format!(
                    "Servizio profili energetici non disponibile: {error}"
                )),
            }
        });
        power.append(&controls);
        power.append(&feedback);
    }
    power.append(&info_row("dialog-information","Inattività e sospensione",
        "I timer di inattività, le azioni del coperchio e la sospensione automatica richiedono ancora l'integrazione dei servizi di sessione Melania."));
    stack.add_titled(&power, Some("power"), "Alimentazione");

    let storage = page("Archiviazione", "drive-harddisk");
    group(&storage, "Dischi");
    let space = root_storage();
    storage.append(&info_row("drive-harddisk", "Filesystem principale", &space));
    storage.append(&info_row("dialog-information", "Gestione delle unità",
        "Le operazioni sui dischi rimangono affidate alle utility del sistema operativo."));
    stack.add_titled(&storage, Some("storage"), "Archiviazione");

    let bluetooth = page("Bluetooth", "bluetooth");
    crate::connectivity_controls::append_bluetooth(&bluetooth);
    group(&bluetooth, "Stato del controller");
    bluetooth.append(&info_row("bluetooth", "Controller Bluetooth",
        &read_command("bluetoothctl", &["show"]).unwrap_or_else(|| "Controller non disponibile o servizio BlueZ non attivo.".into())));
    stack.add_titled(&bluetooth, Some("bluetooth"), "Bluetooth");
    // The network overview is read-only until Melania integrates NetworkManager
    // controls; never misrepresent a setting as applied to the host.


    let applications = page("App", "application-x-executable");
    group(&applications, "Applicazioni");
    applications.append(&info_row("applications-other", "Applicazioni installate",
        "Le applicazioni disponibili compaiono nel menu Start di Melania."));
    applications.append(&info_row("dialog-information", "App predefinite",
        "Le associazioni dei file sono gestite tramite XDG e le impostazioni delle singole applicazioni."));
    stack.add_titled(&applications, Some("applications"), "App");

    let account = page("Account", "avatar-default");
    group(&account, "Utente");
    account.append(&info_row("avatar-default", "Account corrente",
        &env::var("USER").unwrap_or_else(|_| "Utente non disponibile".into())));
    account.append(&info_row("dialog-information", "Sicurezza account",
        "Password e autorizzazioni sono gestite dal sistema Linux; Melania non modifica le credenziali."));
    stack.add_titled(&account, Some("account"), "Account");

    let datetime = page("Data e ora", "x-office-calendar");
    group(&datetime, "Sistema");
    datetime.append(&info_row("x-office-calendar", "Ora del sistema",
        &read_command("date", &["+%d/%m/%Y %H:%M %Z"])
            .unwrap_or_else(|| "Ora non disponibile.".into())));
    datetime.append(&info_row("dialog-information", "Sincronizzazione",
        "Data e fuso orario vengono regolati dai servizi del sistema operativo."));
    stack.add_titled(&datetime, Some("datetime"), "Data e ora");

    let devices = page("Dispositivi", "input-keyboard");
    group(&devices, "Hardware");
    devices.append(&info_row("input-keyboard", "Input e periferiche",
        "Tastiera, mouse e dispositivi USB vengono gestiti dal sottosistema Linux."));
    stack.add_titled(&devices, Some("devices"), "Dispositivi");

    let privacy = page("Privacy e sicurezza", "security-high");
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

    let region = page("Lingua e regione", "preferences-desktop-locale");
    group(&region, "Lingua");
    region.append(&info_row("preferences-desktop-locale", "Lingua della sessione",
        &env::var("LANG").unwrap_or_else(|_| "Locale non disponibile".into())));
    stack.add_titled(&region, Some("region"), "Lingua e regione");

    let about = page("Informazioni", "dialog-information");
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
    use super::{parse_volume,parse_power_profiles};

    #[test]
    fn only_whitelisted_power_profiles_are_parsed() {
        let output="  power-saver:\n  * balanced:\n    Driver: platform_profile\n  performance:\n  malicious;command:\n";
        assert_eq!(parse_power_profiles(output),
            vec!["power-saver","balanced","performance"]);
        assert!(parse_power_profiles("No profiles found.").is_empty());
    }
    #[test]
    fn labels_distinguish_microphone_from_output() {
        assert_eq!(super::audio_mute_label("@DEFAULT_AUDIO_SINK@",false),"Disattiva audio");
        assert_eq!(super::audio_mute_label("@DEFAULT_AUDIO_SINK@",true),"Riattiva audio");
        assert_eq!(super::audio_mute_label("@DEFAULT_AUDIO_SOURCE@",false),"Disattiva microfono");
        assert_eq!(super::audio_mute_label("@DEFAULT_AUDIO_SOURCE@",true),"Attiva microfono");
    }
    #[test]
    fn reads_pipewire_volume_without_exceeding_slider_range() {
        assert_eq!(parse_volume("Volume: 0.42"), Some(42.0));
        assert_eq!(parse_volume("Volume: 1.25 [MUTED]"), Some(100.0));
        assert_eq!(parse_volume("Volume: 0.00"), Some(0.0));
        assert_eq!(parse_volume("No audio backend"), None);
        assert_eq!(parse_volume("Volume: NaN"), None);
    }
}
