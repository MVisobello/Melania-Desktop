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

/// Only a real backlight offers the slider. A 5% floor prevents accidentally
/// making a laptop screen appear entirely black from Settings.
pub fn append_backlight(page:&GtkBox){
    let Some(percent)=backlight_percent() else {
        page.append(&label("Luminosità non disponibile: nessun dispositivo di retroilluminazione controllabile.",
            "settings-row-description"));
        return;
    };
    let row=GtkBox::new(Orientation::Horizontal,9);
    row.add_css_class("settings-action-row");
    let value=Scale::with_range(Orientation::Horizontal,5.0,100.0,1.0);
    value.set_hexpand(true);
    value.set_value(percent.max(5.0));
    value.set_tooltip_text(Some("Luminosità dello schermo interno, dal 5% al 100%"));
    let apply=Button::with_label("Applica luminosità");
    apply.add_css_class("settings-save");
    let refresh=Button::with_label("Aggiorna");
    refresh.add_css_class("settings-save");
    row.append(&value);
    row.append(&apply);
    row.append(&refresh);
    let current=label(&format!("Luminosità attuale: {percent:.0}%"),"settings-row-description");
    let feedback=label("","settings-action-feedback");
    page.append(&current);
    page.append(&row);
    page.append(&feedback);

    let level=value.clone();
    let status=current.clone();
    let message=feedback.clone();
    apply.connect_clicked(move |_|{
        let requested=format!("{}%",level.value().round().clamp(5.0,100.0) as u8);
        let changed=Command::new("brightnessctl")
            .args(["-c","backlight","set",requested.as_str()]).status();
        if !changed.is_ok_and(|result|result.success()){
            message.set_text("Impossibile regolare la luminosità. Verifica dispositivo e permessi.");
            return;
        }
        match backlight_percent(){
            Some(actual)=>{
                level.set_value(actual.max(5.0));
                status.set_text(&format!("Luminosità attuale: {actual:.0}%"));
                message.set_text("Luminosità applicata.");
            }
            None=>message.set_text("Comando inviato, ma il valore non è verificabile."),
        }
    });
    let level=value.clone();
    let status=current.clone();
    let message=feedback.clone();
    refresh.connect_clicked(move |_|{
        match backlight_percent(){
            Some(actual)=>{
                level.set_value(actual.max(5.0));
                status.set_text(&format!("Luminosità attuale: {actual:.0}%"));
                message.set_text("");
            }
            None=>message.set_text("Impossibile leggere la luminosità dello schermo."),
        }
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
