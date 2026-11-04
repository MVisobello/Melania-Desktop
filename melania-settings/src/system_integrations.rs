//! Narrow Rust/GTK4 bridges to mature Linux settings backends.
//!
//! Only an independent Melania labwc login may open these host-mutating
//! tools. Each executable is a static whitelist entry; no shell, user string,
//! desktop file or remote IPC may select arbitrary commands.
use gtk::{Box as GtkBox,Button,Label,Orientation,prelude::*};
use std::{env,fs,os::unix::fs::PermissionsExt,process::Command};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Tool{Displays,AudioMixer,NetworkProfiles,BluetoothDevices,Printers}
impl Tool {
    pub fn program(self)->&'static str {
        match self{
            Self::Displays=>"wdisplays",
            Self::AudioMixer=>"pavucontrol",
            Self::NetworkProfiles=>"nm-connection-editor",
            Self::BluetoothDevices=>"blueman-manager",
            Self::Printers=>"system-config-printer",
        }
    }
    fn label(self)->&'static str{
        match self{
            Self::Displays=>"Configura risoluzione, schermi e scala",
            Self::AudioMixer=>"Mixer avanzato audio e microfono",
            Self::NetworkProfiles=>"Gestisci reti Wi-Fi e VPN",
            Self::BluetoothDevices=>"Associa e gestisci dispositivi Bluetooth",
            Self::Printers=>"Configura stampanti e code di stampa",
        }
    }
    fn package(self)->&'static str{
        match self{
            Self::Displays=>"wdisplays",
            Self::AudioMixer=>"pavucontrol",
            Self::NetworkProfiles=>"network-manager-gnome",
            Self::BluetoothDevices=>"blueman",
            Self::Printers=>"system-config-printer",
        }
    }
}
fn real_session(get:impl Fn(&str)->Option<String>)->bool{
    get("MELANIA_EXTERNAL_COMPOSITOR").as_deref()==Some("labwc")
        && get("MELANIA_NESTED_SESSION").as_deref()!=Some("1")
        && get("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()==Some("1")
        && get("XDG_SESSION_TYPE").as_deref()==Some("wayland")
}
fn executable(program:&str)->bool{
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|base|{
        fs::metadata(base.join(program)).is_ok_and(|meta|
            meta.is_file() && (meta.permissions().mode()&0o111)!=0)
    })
}
/// Installable system utilities provide operational device configuration now,
/// without claiming their controls are native Melania or duplicating CUPS/PAM.
pub fn append_tool(parent:&GtkBox,tool:Tool){
    let row=GtkBox::new(Orientation::Vertical,5);
    row.add_css_class("settings-card");
    let name=Label::new(Some(tool.label()));
    name.set_xalign(0.0);
    name.set_wrap(true);
    name.add_css_class("settings-row-title");
    row.append(&name);
    let helper=Label::new(Some(&format!(
        "Componente Linux: {}. Nessuna modifica a Cinnamon durante le prove.",
        tool.program()
    )));
    helper.set_xalign(0.0);
    helper.set_wrap(true);
    helper.add_css_class("settings-row-description");
    row.append(&helper);
    let button=Button::with_label("Apri configurazione");
    button.set_halign(gtk::Align::Start);
    button.add_css_class("settings-save");
    let allowed=real_session(|k|env::var(k).ok());
    let installed=executable(tool.program());
    button.set_sensitive(allowed && installed);
    if !allowed{
        button.set_tooltip_text(Some(
            "Questa impostazione modifica il sistema: disponibile solo nella sessione Melania principale."
        ));
    }else if !installed{
        button.set_tooltip_text(Some(&format!(
            "Componente assente. Installa il pacchetto {} da Gestore pacchetti.",tool.package()
        )));
    }
    let feedback=Label::new(Some(if !allowed {
        "Non disponibile nell'anteprima annidata."
    } else if !installed {
        "Utilità non installata; apri Gestore pacchetti per abilitarla."
    } else {
        "La configurazione viene applicata dal servizio Linux effettivo."
    }));
    feedback.set_xalign(0.0);
    feedback.set_wrap(true);
    feedback.add_css_class("settings-row-description");
    let status=feedback.clone();
    button.connect_clicked(move |_|{
        // No shell expansion; spawned process is independently managed by
        // the OS and is never killed when the Settings window is closed.
        let command=tool.program();
        match Command::new(command).spawn(){
            Ok(_)=>status.set_text("Strumento di configurazione avviato."),
            Err(error)=>status.set_text(&format!("Impossibile aprire {command}: {error}")),
        }
    });
    row.append(&button);
    row.append(&feedback);
    parent.append(&row);
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn all_settings_backends_are_explicitly_whitelisted(){
        let tools=[Tool::Displays,Tool::AudioMixer,Tool::NetworkProfiles,
            Tool::BluetoothDevices,Tool::Printers];
        for tool in tools{
            assert!(!tool.program().contains(' '));
            assert!(!tool.package().is_empty());
        }
        assert_eq!(Tool::Displays.program(),"wdisplays");
        assert_eq!(Tool::Printers.program(),"system-config-printer");
    }
    #[test]fn previews_cannot_modify_host_hardware_via_gtk_bridge(){
        let full=|k:&str|match k{
            "MELANIA_EXTERNAL_COMPOSITOR"=>Some("labwc".to_owned()),
            "MELANIA_ALLOW_HOST_POWER_ACTIONS"=>Some("1".to_owned()),
            "XDG_SESSION_TYPE"=>Some("wayland".to_owned()),
            _=>None
        };
        assert!(real_session(full));
        assert!(!real_session(|k|if k=="MELANIA_NESTED_SESSION"{
            Some("1".to_owned())}else{full(k)}));
        assert!(!real_session(|k|if k=="MELANIA_EXTERNAL_COMPOSITOR"{
            Some("cinnamon".to_owned())}else{full(k)}));
    }
}
