//! Real backlight control for Melania's Schermo settings.
//! brightnessctl targets the sysfs backlight class only (never keyboard LEDs).
//! Opening Settings performs read-only queries; changes require a button click.
use gtk::{self,prelude::*};
use gtk::{Box as GtkBox,Button,Label,Orientation,Scale};
use std::process::Command;

fn label(message:&str,class:&str)->Label {
    let text=Label::new(Some(message));
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.add_css_class(class);
    text
}
fn raw_brightness(operation:&str)->Option<u64>{
    let output=Command::new("brightnessctl")
        .args(["-c","backlight",operation]).output().ok()?;
    if !output.status.success(){return None;}
    String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()
}
fn parse_percent(current:u64,maximum:u64)->Option<f64>{
    if maximum==0 || current>maximum{return None;}
    Some((current as f64*100.0/maximum as f64).clamp(0.0,100.0))
}
fn backlight_percent()->Option<f64>{
    parse_percent(raw_brightness("get")?,raw_brightness("max")?)
}

/// A real backlight slider is revealed only when brightnessctl detects
/// one. Device detection never blocks GTK page creation.
pub fn append_backlight(page:&GtkBox) {
    let status=label("Rilevamento retroilluminazione in corso…","settings-row-description");
    page.append(&status);
    let row=GtkBox::new(Orientation::Horizontal,9);
    row.add_css_class("settings-action-row");
    let value=Scale::with_range(Orientation::Horizontal,5.0,100.0,1.0);
    value.set_hexpand(true);
    value.set_tooltip_text(Some("Luminosità dello schermo interno, dal 5% al 100%"));
    let apply=Button::with_label("Applica luminosità");
    apply.add_css_class("settings-save");
    let refresh=Button::with_label("Aggiorna");
    refresh.add_css_class("settings-save");
    row.append(&value);
    row.append(&apply);
    row.append(&refresh);
    let current=label("","settings-row-description");
    let feedback=label("","settings-action-feedback");
    let controls=GtkBox::new(Orientation::Vertical,8);
    controls.append(&current);
    controls.append(&row);
    controls.append(&feedback);
    controls.set_visible(false);
    page.append(&controls);

    let ready_status=status.clone();
    let ready_controls=controls.clone();
    let ready_value=value.clone();
    let ready_current=current.clone();
    crate::settings_async::run("backlight-detect",backlight_percent,move |result|{
        match result.flatten() {
            Some(percent)=>{
                ready_value.set_value(percent.max(5.0));
                ready_current.set_text(&format!("Luminosità attuale: {percent:.0}%"));
                ready_controls.set_visible(true);
                ready_status.set_visible(false);
            },
            None=>ready_status.set_text(
                "Luminosità non disponibile: nessun dispositivo di retroilluminazione controllabile."),
        }
    });

    let level=value.clone();
    let current_status=current.clone();
    let message=feedback.clone();
    let refresh_button=refresh.clone();
    apply.connect_clicked(move |button|{
        let requested=format!("{}%",level.value().round().clamp(5.0,100.0) as u8);
        button.set_sensitive(false);
        refresh_button.set_sensitive(false);
        message.set_text("Applicazione luminosità in corso…");
        let action=button.clone();
        let reload=refresh_button.clone();
        let slider=level.clone();
        let reading=current_status.clone();
        let feedback=message.clone();
        // Only this explicit user click may write to the real backlight.
        crate::settings_async::run("backlight-set",move ||{
            let changed=Command::new("brightnessctl")
                .args(["-c","backlight","set",requested.as_str()]).status();
            let success=changed.is_ok_and(|result|result.success());
            (success,if success {backlight_percent()}else{None})
        },move |result|{
            action.set_sensitive(true);
            reload.set_sensitive(true);
            match result {
                Some((true,Some(actual)))=>{
                    slider.set_value(actual.max(5.0));
                    reading.set_text(&format!("Luminosità attuale: {actual:.0}%"));
                    feedback.set_text("Luminosità applicata.");
                },
                Some((true,None))=>feedback.set_text(
                    "Comando inviato, ma il valore non è verificabile."),
                _=>feedback.set_text(
                    "Impossibile regolare la luminosità. Verifica dispositivo e permessi."),
            }
        });
    });
    let level=value.clone();
    let status=current.clone();
    let message=feedback.clone();
    refresh.connect_clicked(move |button|{
        button.set_sensitive(false);
        message.set_text("Lettura luminosità in corso…");
        let action=button.clone();
        let slider=level.clone();
        let reading=status.clone();
        let feedback=message.clone();
        crate::settings_async::run("backlight-refresh",backlight_percent,move |result|{
            action.set_sensitive(true);
            match result.flatten() {
                Some(actual)=>{
                    slider.set_value(actual.max(5.0));
                    reading.set_text(&format!("Luminosità attuale: {actual:.0}%"));
                    feedback.set_text("");
                },
                None=>feedback.set_text("Impossibile leggere la luminosità dello schermo."),
            }
        });
    });
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn valid_brightness_ratio_is_bounded_and_not_divided_by_zero(){
        assert_eq!(parse_percent(50,100),Some(50.0));
        assert_eq!(parse_percent(0,200),Some(0.0));
        assert_eq!(parse_percent(100,100),Some(100.0));
        assert_eq!(parse_percent(120,100),None);
        assert_eq!(parse_percent(1,0),None);
    }
}
