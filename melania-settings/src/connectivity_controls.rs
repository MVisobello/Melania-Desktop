//! Explicit, bounded system controls for NetworkManager and BlueZ.
//! No host service is changed while a Settings page is merely opened.
//! Commands take constant arguments only: never pass a user-provided shell string.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Button, Label, Orientation};
use std::process::Command;

fn label(text: &str, class: &str) -> Label {
    let label=Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}

fn control_row(title: &str, description: &str, button: &Button) -> GtkBox {
    let row=GtkBox::new(Orientation::Horizontal,12);
    row.add_css_class("settings-card");
    let description_column=GtkBox::new(Orientation::Vertical,4);
    description_column.set_hexpand(true);
    description_column.append(&label(title,"settings-row-title"));
    description_column.append(&label(description,"settings-row-description"));
    row.append(&description_column);
    button.add_css_class("settings-save");
    button.set_valign(gtk::Align::Center);
    row.append(button);
    row
}

fn parse_wifi_radio(output: &str) -> Option<bool> {
    match output.trim() {
        "enabled" => Some(true),
        "disabled" => Some(false),
        _ => None,
    }
}
fn wifi_radio() -> Option<bool> {
    let output=Command::new("nmcli").args(["radio","wifi"]).output().ok()?;
    if !output.status.success() {return None;}
    parse_wifi_radio(&String::from_utf8_lossy(&output.stdout))
}

fn parse_bluetooth_power(output: &str) -> Option<bool> {
    // bluetoothctl show is controller-specific. A missing controller is not
    // silently treated as an available, powered-off radio.
    if !output.lines().any(|line|line.trim_start().starts_with("Controller ")) {
        return None;
    }
    output.lines().find_map(|line| {
        match line.trim().strip_prefix("Powered: ")?.trim() {
            "yes" => Some(true),
            "no" => Some(false),
            _ => None,
        }
    })
}
fn bluetooth_power() -> Option<bool> {
    let output=Command::new("bluetoothctl").arg("show").output().ok()?;
    if !output.status.success() {return None;}
    parse_bluetooth_power(&String::from_utf8_lossy(&output.stdout))
}

/// Controls are offered only when NetworkManager exposes its Wi-Fi radio.
/// Reading the page does not enable Wi-Fi or disconnect a running connection.
pub fn append_wifi(page: &GtkBox) {
    let Some(enabled)=wifi_radio() else {
        page.append(&label("Controllo Wi-Fi non disponibile tramite NetworkManager.",
            "settings-row-description"));
        return;
    };
    let button=Button::with_label(if enabled {"Disattiva Wi-Fi"} else {"Attiva Wi-Fi"});
    button.set_tooltip_text(Some(
        "Modifica la radio Wi-Fi di Linux; disattivarla può interrompere la connessione."
    ));
    let row=control_row("Wi-Fi",
        "Attiva o disattiva la radio Wi-Fi del sistema. Una disattivazione interrompe le connessioni wireless.",
        &button);
    page.append(&row);
    let feedback=label("", "settings-action-feedback");
    page.append(&feedback);
    button.connect_clicked(move |button| {
        let Some(current)=wifi_radio() else {
            feedback.set_text("Impossibile leggere lo stato Wi-Fi.");
            return;
        };
        let next=if current {"off"} else {"on"};
        match Command::new("nmcli").args(["radio","wifi",next]).status() {
            Ok(status) if status.success() => {
                match wifi_radio() {
                    Some(actual) => {
                        button.set_label(if actual {"Disattiva Wi-Fi"} else {"Attiva Wi-Fi"});
                        feedback.set_text(if actual {"Wi-Fi attivo."} else {"Wi-Fi disattivato."});
                    }
                    None => feedback.set_text("Comando inviato, ma lo stato Wi-Fi non è verificabile."),
                }
            }
            _ => feedback.set_text("Impossibile cambiare il Wi-Fi. Verifica NetworkManager e i permessi."),
        }
    });
}

/// Use BlueZ to control the current default adapter. This is an explicit
/// user action, never an automatic side effect of opening Settings.
pub fn append_bluetooth(page: &GtkBox) {
    let Some(enabled)=bluetooth_power() else {
        page.append(&label("Nessun controller Bluetooth gestibile tramite BlueZ.",
            "settings-row-description"));
        return;
    };
    let button=Button::with_label(if enabled {"Disattiva Bluetooth"} else {"Attiva Bluetooth"});
    button.set_tooltip_text(Some("Accende o spegne l'adattatore Bluetooth predefinito di Linux."));
    page.append(&control_row("Bluetooth",
        "Controlla l'adattatore predefinito. Spegnerlo disconnette i dispositivi associati.",
        &button));
    let feedback=label("", "settings-action-feedback");
    page.append(&feedback);
    button.connect_clicked(move |button| {
        let Some(current)=bluetooth_power() else {
            feedback.set_text("Impossibile leggere lo stato dell'adattatore Bluetooth.");
            return;
        };
        let next=if current {"off"} else {"on"};
        match Command::new("bluetoothctl").args(["power",next]).status() {
            Ok(status) if status.success() => {
                match bluetooth_power() {
                    Some(actual) => {
                        button.set_label(if actual {"Disattiva Bluetooth"} else {"Attiva Bluetooth"});
                        feedback.set_text(if actual {"Bluetooth attivo."} else {"Bluetooth disattivato."});
                    }
                    None => feedback.set_text("Comando inviato, ma il controller non risponde."),
                }
            }
            _ => feedback.set_text("Impossibile cambiare il Bluetooth. Verifica BlueZ e i permessi."),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wifi_state_is_strictly_parsed() {
        assert_eq!(parse_wifi_radio("enabled\n"),Some(true));
        assert_eq!(parse_wifi_radio("disabled"),Some(false));
        assert_eq!(parse_wifi_radio("unavailable"),None);
        assert_eq!(parse_wifi_radio("enabled; echo unsafe"),None);
    }
    #[test]
    fn bluetooth_requires_a_real_controller_and_boolean_power() {
        assert_eq!(parse_bluetooth_power("Controller AA:BB:CC test\n    Powered: yes\n"),Some(true));
        assert_eq!(parse_bluetooth_power("Controller AA:BB:CC test\n    Powered: no\n"),Some(false));
        assert_eq!(parse_bluetooth_power("Powered: yes"),None);
        assert_eq!(parse_bluetooth_power("Controller aa\nPowered: maybe"),None);
    }
}
