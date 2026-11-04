//! Real libinput touchpad/mouse preferences via Rust melania-inputctl.
use gtk::{prelude::*, Align,Box as GtkBox,Button,Label,Orientation,Scale,Switch};
use std::{env,process::Command};

#[derive(Clone,Copy,Debug)]
struct InputState{tap:bool,natural:bool,left:bool,speed:i32}
impl Default for InputState{
    fn default()->Self{Self{tap:true,natural:false,left:false,speed:0}}
}
fn parse(stdout:&str)->Option<InputState>{
    let mut state=InputState::default();
    for line in stdout.lines(){
        let (name,val)=line.split_once('=')?;
        match name{
            "tap"=>state.tap=val=="yes",
            "natural_scroll"=>state.natural=val=="yes",
            "left_handed"=>state.left=val=="yes",
            "speed"=>state.speed=val.parse::<i32>().ok()?,
            _=>return None,
        }
    }
    if !(-100..=100).contains(&state.speed){return None;}
    Some(state)
}
fn real_session()->bool{
    env::var("MELANIA_EXTERNAL_COMPOSITOR").as_deref()==Ok("labwc")
        && env::var("MELANIA_NESTED_SESSION").as_deref()!=Ok("1")
        && env::var("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()==Ok("1")
}
fn heading(title:&str)->Label{
    let w=Label::new(Some(title));w.set_xalign(0.0);w.add_css_class("settings-row-title");w
}
fn bool_row(title:&str,enabled:bool)->(GtkBox,Switch){
    let row=GtkBox::new(Orientation::Horizontal,14);
    row.add_css_class("settings-card");
    let description=heading(title);description.set_hexpand(true);row.append(&description);
    let toggle=Switch::builder().active(enabled).build();
    toggle.set_valign(Align::Center);
    row.append(&toggle);(row,toggle)
}
pub fn append(parent:&GtkBox){
    let title=heading("Mouse e touchpad");
    title.add_css_class("settings-section-heading");
    parent.append(&title);
    let current=Command::new("melania-inputctl").arg("show").output().ok()
        .filter(|o|o.status.success())
        .and_then(|o|parse(&String::from_utf8_lossy(&o.stdout)));
    let defaults=current.unwrap_or_default();
    let supported=real_session()&&current.is_some();
    let (tap_row,tap)=bool_row("Tocco per clic sul touchpad",defaults.tap);
    let (scroll_row,scroll)=bool_row("Scorrimento naturale sul touchpad",defaults.natural);
    let (left_row,left)=bool_row("Pulsante principale a sinistra",!defaults.left);
    for (row,widget) in [(tap_row,tap.clone()),(scroll_row,scroll.clone()),(left_row,left.clone())]{
        widget.set_sensitive(supported);
        parent.append(&row);
    }
    let speed_row=GtkBox::new(Orientation::Vertical,5);
    speed_row.add_css_class("settings-card");
    speed_row.append(&heading("Velocità del puntatore (mouse)"));
    let speed=Scale::with_range(Orientation::Horizontal,-100.0,100.0,5.0);
    speed.set_draw_value(true);
    speed.set_value(defaults.speed as f64);
    speed.set_sensitive(supported);
    speed_row.append(&speed);
    parent.append(&speed_row);
    let info=Label::new(Some(if supported {
        "Le modifiche si applicano solo alla sessione Melania, con labwc --reconfigure."
    }else{
        "Avvia la sessione Melania principale per configurare i dispositivi. Cinnamon non viene modificato."
    }));
    info.set_xalign(0.0);info.set_wrap(true);
    info.add_css_class("settings-row-description");
    parent.append(&info);
    let apply=Button::with_label("Applica mouse e touchpad");
    apply.set_sensitive(supported);
    apply.add_css_class("settings-save");
    apply.set_halign(Align::Start);
    let action=apply.clone();
    apply.connect_clicked(move |_|{
        action.set_sensitive(false);
        let args=[
            if tap.is_active(){"yes".to_owned()}else{"no".to_owned()},
            if scroll.is_active(){"yes".to_owned()}else{"no".to_owned()},
            if left.is_active(){"no".to_owned()}else{"yes".to_owned()},
            (speed.value().round() as i32).to_string(),
        ];
        let message=info.clone();
        let button=action.clone();
        crate::settings_async::run("libinput-apply",move ||{
            Command::new("melania-inputctl").arg("apply").args(args)
                .output().map(|o|o.status.success()).unwrap_or(false)
        },move |response|{
            message.set_text(if response==Some(true){
                "Impostazioni di mouse e touchpad applicate e salvate per Melania."
            }else{
                "Modifica non applicata: controlla la sessione o i permessi di labwc."
            });
            button.set_sensitive(true);
        });
    });
    parent.append(&apply);
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn input_output_is_strict_and_no_external_values_drive_commands(){
        let p=parse("tap=no\nnatural_scroll=yes\nleft_handed=yes\nspeed=-20\n").unwrap();
        assert!(!p.tap&&p.natural&&p.left&&p.speed==-20);
        assert!(parse("speed=101").is_none());
        assert!(parse("bad=evil").is_none());
    }
}
