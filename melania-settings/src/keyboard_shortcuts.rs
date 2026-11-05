//! Configurable compositor shortcuts for the standalone labwc session.
//! Never changes XFCE, Cinnamon, user labwc config or the nested preview.
//! Persisted and validated by the Rust melania-inputctl backend.
use gtk::{prelude::*, Align,Box as GtkBox,Button,DropDown,Label,Orientation};
use std::{collections::HashSet,env,process::Command};

const ACTIONS:[(&str,&str);5]=[
    ("start","Apri menu Start"),
    ("files","Apri Melania Files"),
    ("settings","Apri Impostazioni"),
    ("lock","Blocca la sessione"),
    ("terminal","Apri Terminale"),
];
const KEYS:[&str;12]=[
    "W-space","W-e","W-i","W-l","W-Return","W-f",
    "W-k","W-d","W-t","W-p","C-A-e","C-A-t",
];
const LABELS:[&str;12]=[
    "Super + Spazio","Super + E","Super + I","Super + L","Super + Invio",
    "Super + F","Super + K","Super + D","Super + T","Super + P",
    "Ctrl + Alt + E","Ctrl + Alt + T",
];
fn label(value:&str,class:&str)->Label{
    let w=Label::new(Some(value));
    w.set_xalign(0.0);
    w.set_wrap(true);
    w.add_css_class(class);
    w
}
fn read_config(output:&str)->Option<Vec<String>>{
    let mut entries=std::collections::HashMap::new();
    for line in output.lines(){
        let (key,value)=line.split_once('=')?;
        if !ACTIONS.iter().any(|(name,_)|*name==key) ||
            !KEYS.contains(&value) || entries.insert(key,value).is_some(){
            return None;
        }
    }
    if entries.len()!=ACTIONS.len(){return None;}
    let chosen=ACTIONS.iter().map(|(key,_)|entries.get(key).copied().unwrap_or(""))
        .collect::<Vec<_>>();
    if chosen.iter().copied().collect::<HashSet<_>>().len()!=ACTIONS.len(){
        return None;
    }
    Some(chosen.into_iter().map(str::to_owned).collect())
}
fn standalone()->bool{
    env::var("MELANIA_EXTERNAL_COMPOSITOR").as_deref()==Ok("labwc")
        && env::var("MELANIA_NESTED_SESSION").as_deref()!=Ok("1")
        && env::var("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()==Ok("1")
        && env::var("WAYLAND_DISPLAY").is_ok()
}
pub fn append(parent:&GtkBox){
    parent.append(&label("Scorciatoie della sessione","settings-section-heading"));
    let actual=Command::new("melania-inputctl").arg("shortcuts-show").output()
        .ok().filter(|output|output.status.success())
        .and_then(|output|read_config(&String::from_utf8_lossy(&output.stdout)));
    let defaults=["W-space","W-e","W-i","W-l","W-Return"];
    let configured=actual.clone().unwrap_or_else(||defaults.map(str::to_owned).to_vec());
    let supported=standalone() && actual.is_some();
    let mut pickers=Vec::new();
    for (i,(_,name)) in ACTIONS.iter().enumerate(){
        let row=GtkBox::new(Orientation::Horizontal,14);
        row.add_css_class("settings-card");
        let title=label(name,"settings-row-title");
        title.set_hexpand(true);
        row.append(&title);
        let picker=DropDown::from_strings(&LABELS);
        let index=KEYS.iter().position(|key|*key==configured[i]).unwrap_or(i);
        picker.set_selected(index as u32);
        picker.set_sensitive(supported);
        picker.set_hexpand(false);
        picker.set_halign(Align::End);
        row.append(&picker);
        parent.append(&row);
        pickers.push(picker);
    }
    let message=label(if supported{
        "Le nuove combinazioni sono applicate a labwc; anche le impostazioni mouse restano invariate."
    }else{
        "Modifiche disponibili solo nella sessione principale Melania con labwc. Nessuna scorciatoia dell'ambiente ospitante viene modificata."
    },"settings-row-description");
    parent.append(&message);
    let apply=Button::with_label("Applica scorciatoie");
    apply.add_css_class("settings-save");
    apply.set_halign(Align::Start);
    apply.set_sensitive(supported);
    let apply_button=apply.clone();
    apply.connect_clicked(move |_|{
        let chosen=pickers.iter().map(|picker|KEYS[picker.selected() as usize].to_owned())
            .collect::<Vec<_>>();
        if chosen.iter().collect::<HashSet<_>>().len()!=ACTIONS.len(){
            message.set_text("Ogni azione deve avere una combinazione diversa.");
            return;
        }
        apply_button.set_sensitive(false);
        message.set_text("Applicazione delle scorciatoie in corso…");
        let message=message.clone();
        let apply_button=apply_button.clone();
        crate::settings_async::run("keyboard-shortcuts",move ||{
            Command::new("melania-inputctl").arg("shortcuts-apply").args(chosen)
                .output().map(|output|output.status.success()).unwrap_or(false)
        },move |result|{
            message.set_text(if result==Some(true){
                "Scorciatoie salvate e ricaricate nella sessione Melania."
            }else{
                "Non applicate: verifica la sessione labwc e la configurazione locale."
            });
            apply_button.set_sensitive(true);
        });
    });
    parent.append(&apply);
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn reject_duplicate_or_invalid_shortcut_output(){
        let value="start=W-space\nfiles=W-e\nsettings=W-i\nlock=W-l\nterminal=W-Return\n";
        assert_eq!(read_config(value).unwrap().len(),5);
        assert!(read_config("start=W-space\nfiles=W-space\nsettings=W-i\nlock=W-l\nterminal=W-Return\n").is_none());
        assert!(read_config("start=A-F4\nfiles=W-e\nsettings=W-i\nlock=W-l\nterminal=W-Return\n").is_none());
        assert!(read_config("unknown=W-e\n").is_none());
    }
}
