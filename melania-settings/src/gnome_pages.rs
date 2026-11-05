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
/// Apply a verified PipeWire snapshot on GTK's owning thread.
fn apply_audio_snapshot(target:&str, snapshot:(f64,bool), slider:&Scale,
                        mute:&Button, reading:&Label) {
    let (volume,muted)=snapshot;
    slider.set_value(volume);
    mute.set_label(audio_mute_label(target,muted));
    reading.set_text(&format!("Volume attuale: {volume:.0}%{}",
        if muted {" · disattivato"}else{""}));
}

/// PipeWire can be slow or unavailable on software-rendered VMs. All status
/// queries and explicit writes run through a bounded worker. Widgets are
/// built immediately but remain hidden until a real device is confirmed.
fn audio_device_controls(page:&GtkBox, title:&str, target:&'static str) {
    group(page,title);
    let status=label("Rilevamento dispositivo audio…","settings-row-description");
    page.append(&status);
    let level=Scale::with_range(Orientation::Horizontal,0.0,100.0,1.0);
    level.set_hexpand(true);
    level.set_tooltip_text(Some("Volume del dispositivo in percentuale"));
    let apply=Button::with_label("Applica volume");
    apply.add_css_class("settings-save");
    let mute=Button::with_label("Disattiva audio");
    mute.add_css_class("settings-save");
    let refresh=Button::with_label("Aggiorna");
    refresh.add_css_class("settings-save");
    let reading=label("","settings-row-description");
    let feedback=label("","settings-action-feedback");
    let buttons=GtkBox::new(Orientation::Horizontal,8);
    buttons.add_css_class("settings-action-row");
    buttons.append(&level);
    buttons.append(&apply);
    buttons.append(&mute);
    buttons.append(&refresh);
    let controls=GtkBox::new(Orientation::Vertical,8);
    controls.append(&reading);
    controls.append(&buttons);
    controls.append(&feedback);
    controls.set_visible(false);
    page.append(&controls);

    let ready_controls=controls.clone();
    let ready_status=status.clone();
    let ready_level=level.clone();
    let ready_mute=mute.clone();
    let ready_reading=reading.clone();
    let unavailable=title.to_owned();
    crate::settings_async::run("audio-detect",move ||audio_status(target),move |result|{
        match result.flatten(){
            Some(snapshot)=>{
                apply_audio_snapshot(target,snapshot,&ready_level,&ready_mute,&ready_reading);
                ready_controls.set_visible(true);
                ready_status.set_visible(false);
            },
            None=>ready_status.set_text(&format!(
                "{unavailable}: dispositivo PipeWire/WirePlumber non disponibile; nessun controllo fittizio.")),
        }
    });

    let slider=level.clone();
    let mute_button=mute.clone();
    let refresh_button=refresh.clone();
    let state=reading.clone();
    let message=feedback.clone();
    apply.connect_clicked(move |button|{
        // Only the user's explicit click may change host mixer state.
        let requested=format!("{:.2}",slider.value().clamp(0.0,100.0)/100.0);
        button.set_sensitive(false);
        mute_button.set_sensitive(false);
        refresh_button.set_sensitive(false);
        message.set_text("Applicazione volume in corso…");
        let action=button.clone();
        let other=mute_button.clone();
        let reload=refresh_button.clone();
        let slider=slider.clone();
        let mute=other.clone();
        let reading=state.clone();
        let feedback=message.clone();
        crate::settings_async::run("audio-volume",move ||{
            let result=Command::new("wpctl")
                .args(["set-volume",target,requested.as_str()]).status();
            let ok=result.is_ok_and(|status|status.success());
            (ok,if ok {audio_status(target)}else{None})
        },move |response|{
            action.set_sensitive(true);
            other.set_sensitive(true);
            reload.set_sensitive(true);
            match response{
                Some((true,Some(snapshot)))=>{
                    apply_audio_snapshot(target,snapshot,&slider,&mute,&reading);
                    feedback.set_text("Volume applicato.");
                },
                Some((true,None))=>feedback.set_text(
                    "Comando eseguito, ma non è possibile verificare il volume."),
                _=>feedback.set_text(
                    "Impossibile applicare il volume. Verifica WirePlumber."),
            }
        });
    });

    let slider=level.clone();
    let apply_button=apply.clone();
    let refresh_button=refresh.clone();
    let state=reading.clone();
    let message=feedback.clone();
    mute.connect_clicked(move |button|{
        button.set_sensitive(false);
        apply_button.set_sensitive(false);
        refresh_button.set_sensitive(false);
        message.set_text("Aggiornamento audio in corso…");
        let action=button.clone();
        let other=apply_button.clone();
        let reload=refresh_button.clone();
        let slider=slider.clone();
        let reading=state.clone();
        let feedback=message.clone();
        crate::settings_async::run("audio-mute",move ||{
            let result=Command::new("wpctl").args(["set-mute",target,"toggle"]).status();
            let ok=result.is_ok_and(|status|status.success());
            (ok,if ok {audio_status(target)}else{None})
        },move |response|{
            action.set_sensitive(true);
            other.set_sensitive(true);
            reload.set_sensitive(true);
            match response{
                Some((true,Some(snapshot)))=>{
                    apply_audio_snapshot(target,snapshot,&slider,&action,&reading);
                    feedback.set_text("Stato audio aggiornato.");
                },
                Some((true,None))=>feedback.set_text(
                    "Comando eseguito, ma il dispositivo non risponde."),
                _=>feedback.set_text("Impossibile modificare lo stato audio."),
            }
        });
    });

    let slider=level.clone();
    let mute_button=mute.clone();
    let state=reading.clone();
    let message=feedback.clone();
    refresh.connect_clicked(move |_|{
        message.set_text("Lettura dispositivo in corso…");
        let slider=slider.clone();
        let mute=mute_button.clone();
        let reading=state.clone();
        let feedback=message.clone();
        crate::settings_async::run("audio-refresh",move ||audio_status(target),move |response|{
            match response.flatten(){
                Some(snapshot)=>{
                    apply_audio_snapshot(target,snapshot,&slider,&mute,&reading);
                    feedback.set_text("");
                },
                None=>feedback.set_text("Impossibile leggere il dispositivo audio."),
            }
        });
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
pub const LAZY_PAGE_NAMES: [(&str,&str); 12] = [
    ("display", "Schermo"),
    ("audio", "Audio"),
    ("power", "Alimentazione"),
    ("storage", "Archiviazione"),
    ("bluetooth", "Bluetooth"),
    ("applications", "App"),
    ("account", "Account"),
    ("datetime", "Data e ora"),
    ("devices", "Dispositivi"),
    ("privacy", "Privacy"),
    ("region", "Lingua e regione"),
    ("about", "Informazioni"),
];

/// Register a stable child for each category before GTK first paint.
/// The heavy controls are built only when the user navigates there.
pub fn add_pages(stack:&Stack) {
    crate::lazy_pages::register(stack, "display", "Schermo");
    crate::lazy_pages::register(stack, "audio", "Audio");
    crate::lazy_pages::register(stack, "power", "Alimentazione");
    crate::lazy_pages::register(stack, "storage", "Archiviazione");
    crate::lazy_pages::register(stack, "bluetooth", "Bluetooth");
    crate::lazy_pages::register(stack, "applications", "App");
    crate::lazy_pages::register(stack, "account", "Account");
    crate::lazy_pages::register(stack, "datetime", "Data e ora");
    crate::lazy_pages::register(stack, "devices", "Dispositivi");
    crate::lazy_pages::register(stack, "privacy", "Privacy");
    crate::lazy_pages::register(stack, "region", "Lingua e regione");
    crate::lazy_pages::register(stack, "about", "Informazioni");
}

/// Construct exactly the selected real category, retaining its original
/// controls and signal handlers. Other pages remain inert until selected.
pub fn build_page(stack:&Stack, key:&str)->Option<GtkBox> {
    match key {
        "display" => {
        let display = page("Schermo", "video-display");
        crate::display_controls::append_backlight(&display);
        let monitor_count = gtk::gdk::Display::default()
            .map(|d| d.monitors().n_items()).unwrap_or(0);
        group(&display, "Monitor");
        crate::system_integrations::append_tool(&display,crate::system_integrations::Tool::Displays);
        display.append(&info_row("video-display", "Schermi rilevati",
            &format!("{monitor_count} monitor rilevati da GTK nella sessione corrente.")));
        display.append(&info_row("dialog-information", "Risoluzione e scala",
            "La configurazione dei monitor richiede ancora un'interfaccia al compositore Melania. Nessuna modifica è applicata al sistema ospitante."));
        group(&display, "Rotazione dello schermo");
        display.append(&info_row("video-display", "Sensore di orientamento",
            "Disponibilità del sensore interrogata tramite iio-sensor-proxy."));
        let sensor_status=label("Rilevamento accelerometro in corso…","settings-row-description");
        sensor_status.add_css_class("settings-card");
        display.append(&sensor_status);
        let initial_status=sensor_status.clone();
        crate::settings_async::run("display-sensor",sensors::accelerometer,move |result|{
            initial_status.set_text(sensors::explanation(
                result.unwrap_or(sensors::Accelerometer::Unknown)));
        });
        let sensor_refresh=Button::with_label("Rileva nuovamente i sensori");
        sensor_refresh.add_css_class("settings-save");
        sensor_refresh.set_halign(Align::Start);
        let sensor_description=sensor_status.clone();
        sensor_refresh.connect_clicked(move |button| {
            button.set_sensitive(false);
            sensor_description.set_text("Rilevamento accelerometro in corso…");
            let label=sensor_description.clone();
            let refresh=button.clone();
            crate::settings_async::run("display-sensor-refresh",sensors::accelerometer,move |result|{
                label.set_text(sensors::explanation(
                    result.unwrap_or(sensors::Accelerometer::Unknown)));
                refresh.set_sensitive(true);
            });
        });
        display.append(&sensor_refresh);
        display.append(&info_row("dialog-information", "Rotazione manuale e blocco",
            "Non ancora disponibili: occorre una connessione al compositore Wayland, con sincronizzazione di touchscreen, finestre e area utile. Nessun comando esterno modifica lo schermo."));
        Some(display)
        },
        "audio" => {
        let audio = page("Audio", "audio-card");
        // Input and output are separate PipeWire nodes. A missing device gets a
        // clear unavailable state; no fake slider or host-wide mixer fallback.
        audio_device_controls(&audio, "Uscita audio", "@DEFAULT_AUDIO_SINK@");
        audio_device_controls(&audio, "Microfono", "@DEFAULT_AUDIO_SOURCE@");
        crate::system_integrations::append_tool(&audio,crate::system_integrations::Tool::AudioMixer);
        Some(audio)
        },
        "power" => {
        let power=page("Alimentazione","battery");
        group(&power,"Batteria");
        let battery_reading=GtkBox::new(Orientation::Vertical,0);
        battery_reading.append(&info_row("battery","Stato dell'alimentazione",
            "Rilevamento batteria in corso…"));
        power.append(&battery_reading);
        group(&power,"Profili energetici");
        let profiles_host=GtkBox::new(Orientation::Vertical,8);
        let pending=label("Lettura profili energetici in corso…",
            "settings-row-description");
        profiles_host.append(&pending);
        power.append(&profiles_host);
        power.append(&info_row("dialog-information","Inattività e sospensione",
            "I timer di inattività, le azioni del coperchio e la sospensione automatica richiedono ancora l'integrazione dei servizi di sessione Melania."));
        // powerprofilesctl can wait on D-Bus; only plain data is sent
        // from the worker to GTK. No profile is changed on page open.
        crate::settings_async::run("power-detect",move ||{
            let battery=battery_status();
            let list=Command::new("powerprofilesctl").arg("list").output().ok()
                .filter(|result|result.status.success())
                .map(|result|parse_power_profiles(&String::from_utf8_lossy(&result.stdout)))
                .unwrap_or_default();
            let active=if list.is_empty() {String::new()}else{
                read_command("powerprofilesctl",&["get"]).unwrap_or_default()
            };
            (battery,list,active)
        },move |result|{
            let Some((battery,profile_list,active_profile))=result else {
                pending.set_text("Impossibile leggere i profili energetici. Riprova riaprendo la sezione.");
                return;
            };
            // The real battery status is a fresh, read-only Linux sysfs scan.
            while let Some(child)=battery_reading.first_child(){
                battery_reading.remove(&child);
            }
            battery_reading.append(&info_row("battery","Stato dell'alimentazione",&battery));
            profiles_host.remove(&pending);
            if profile_list.is_empty() {
                profiles_host.append(&info_row("dialog-information","Profili non disponibili",
                    "power-profiles-daemon non è disponibile o non espone profili supportati."));
                return;
            }
            let labels=profile_list.iter().map(|name|match *name {
                "power-saver"=>"Risparmio energetico",
                "balanced"=>"Bilanciato",
                "performance"=>"Prestazioni elevate",
                _=>*name,
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
            let feedback=label(
                "La scelta modifica il profilo energetico Linux, anche nelle altre sessioni.",
                "settings-row-description");
            let response=feedback.clone();
            apply.connect_clicked(move |button|{
                let Some(&profile)=profile_list.get(selector.selected() as usize)
                    else {return;};
                button.set_sensitive(false);
                response.set_text("Applicazione profilo energetico in corso…");
                let action=button.clone();
                let feedback=response.clone();
                crate::settings_async::run("power-set",move ||{
                    let result=Command::new("powerprofilesctl").args(["set",profile]).status();
                    let success=result.is_ok_and(|status|status.success());
                    (success,if success {
                        read_command("powerprofilesctl",&["get"])
                    }else{None})
                },move |result|{
                    action.set_sensitive(true);
                    match result{
                        Some((true,Some(actual))) if actual==profile=>
                            feedback.set_text(&format!("Profilo energetico applicato: {profile}.")),
                        Some((true,_))=>feedback.set_text(
                            "Comando inviato, ma non è possibile verificare il profilo attivo."),
                        _=>feedback.set_text(
                            "Impossibile applicare il profilo energetico. Verifica power-profiles-daemon."),
                    }
                });
            });
            profiles_host.append(&controls);
            profiles_host.append(&feedback);
        });
        Some(power)
        },
        "storage" => {
        let storage = page("Archiviazione", "drive-harddisk");
        group(&storage, "Dischi");
        let space=label("Lettura filesystem principale in corso…","settings-row-description");
        space.add_css_class("settings-card");
        storage.append(&space);
        let initial_space=space.clone();
        crate::settings_async::run("storage-df",root_storage,move |result|{
            initial_space.set_text(&format!("Filesystem principale · {}",
                result.as_deref().unwrap_or("Impossibile leggere lo spazio disponibile.")));
        });
        let refresh=Button::with_label("Aggiorna spazio disponibile");
        refresh.add_css_class("settings-save");
        refresh.set_halign(Align::Start);
        let space_label=space.clone();
        refresh.connect_clicked(move |button|{
            button.set_sensitive(false);
            space_label.set_text("Aggiornamento spazio disponibile in corso…");
            let display=space_label.clone();
            let action=button.clone();
            crate::settings_async::run("storage-refresh",root_storage,move |result|{
                display.set_text(&format!("Filesystem principale · {}",
                    result.as_deref().unwrap_or("Impossibile leggere lo spazio disponibile.")));
                action.set_sensitive(true);
            });
        });
        storage.append(&refresh);
        storage.append(&info_row("dialog-information", "Gestione delle unità",
            "Le operazioni sui dischi rimangono affidate alle utility del sistema operativo."));
        Some(storage)
        },
        "bluetooth" => {
        let bluetooth = page("Bluetooth", "bluetooth");
        crate::connectivity_controls::append_bluetooth(&bluetooth);
        crate::system_integrations::append_tool(&bluetooth,crate::system_integrations::Tool::BluetoothDevices);
        group(&bluetooth, "Stato del controller");
        let controller=label("Rilevamento controller in corso…","settings-row-description");
        controller.add_css_class("settings-card");
        bluetooth.append(&controller);
        let info=controller.clone();
        crate::settings_async::run("bluetooth-info",move ||{
            read_command("bluetoothctl",&["show"])
        },move |result|{
            info.set_text(&format!("Controller Bluetooth · {}",result.flatten()
                .unwrap_or_else(||"Controller non disponibile o servizio BlueZ non attivo.".into())));
        });
        Some(bluetooth)
        },
        "applications" => {
        let applications = page("App", "application-x-executable");
        group(&applications, "Applicazioni");
        applications.append(&info_row("applications-other", "Applicazioni installate",
            "Le applicazioni disponibili compaiono nel menu Start di Melania."));
        // Put real controls one click away from their logical parent category,
        // not only in a long left sidebar or the generic System overview.
        applications.append(&linked_row(stack, "xfce-preferred", "applications-default",
            "App predefinite", "Browser e posta elettronica"));
        applications.append(&linked_row(stack, "xfce-mime", "text-x-generic",
            "Associazioni file", "Scegli l'applicazione per ogni tipo di file"));
        applications.append(&linked_row(stack, "xfce-session", "system-run",
            "Avvio automatico", "Applicazioni all'avvio della sessione"));
        Some(applications)
        },
        "account" => {
        let account = page("Account", "avatar-default");
        group(&account, "Utente");
        account.append(&info_row("avatar-default", "Account corrente",
            &env::var("USER").unwrap_or_else(|_| "Utente non disponibile".into())));
        account.append(&info_row("dialog-information", "Sicurezza account",
            "Password e autorizzazioni sono gestite dal sistema Linux; Melania non modifica le credenziali."));
        Some(account)
        },
        "datetime" => {
        let datetime = page("Data e ora", "x-office-calendar");
        group(&datetime, "Sistema");
        datetime.append(&info_row("x-office-calendar", "Ora del sistema",
            &read_command("date", &["+%d/%m/%Y %H:%M %Z"])
                .unwrap_or_else(|| "Ora non disponibile.".into())));
        datetime.append(&info_row("dialog-information", "Sincronizzazione",
            "Data e fuso orario vengono regolati dai servizi del sistema operativo."));
        Some(datetime)
        },
        "devices" => {
        let devices = page("Dispositivi", "input-keyboard");
        group(&devices, "Hardware");
        crate::input_controls::append(&devices);
        crate::system_integrations::append_tool(&devices,crate::system_integrations::Tool::Printers);
        devices.append(&info_row("input-keyboard", "Input e periferiche",
            "Tastiera, mouse e dispositivi USB vengono gestiti dal sottosistema Linux."));
        Some(devices)
        },
        "privacy" => {
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
        Some(privacy)
        },
        "region" => {
        let region = page("Lingua e regione", "preferences-desktop-locale");
        group(&region, "Lingua");
        region.append(&info_row("preferences-desktop-locale", "Lingua della sessione",
            &env::var("LANG").unwrap_or_else(|_| "Locale non disponibile".into())));
        Some(region)
        },
        "about" => {
        let about = page("Informazioni", "dialog-information");
        group(&about, "Versione");
        about.append(&info_row("computer", "Melania Desktop",
            &format!("Versione {} · progetto sperimentale GTK 4 / Wayland",
                     env!("CARGO_PKG_VERSION"))));
        about.append(&info_row("computer", "Sistema Linux",
            &format!("{} · architettura {}", os_name(), env::consts::ARCH)));
        about.append(&info_row("dialog-information", "Stato del progetto",
            "Alcune pagine sono informative. Le opzioni che possono essere cambiate sono contrassegnate da controlli reali."));
        Some(about)
        },
        _ => None,
    }
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
