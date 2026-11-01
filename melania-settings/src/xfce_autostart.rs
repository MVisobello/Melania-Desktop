//! Manage per-user XDG autostart entries, without running Exec lines.
//!
//! The system-wide /etc/xdg/autostart layer and third-party startup daemons
//! are *not* altered. Only existing regular ~/.config/autostart/*.desktop
//! user files can be enabled/disabled. Changes are explicit and atomic.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Label, Orientation, Switch};
use std::{cell::Cell, fs, io::{self,Write},
    os::unix::{fs::OpenOptionsExt,fs::FileTypeExt},
    path::{Path,PathBuf}, rc::Rc};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AutostartItem {pub file_name:String,pub name:String,pub enabled:bool}

fn autostart_dir()->Option<PathBuf>{
    let config=std::env::var_os("XDG_CONFIG_HOME").filter(|x|!x.is_empty())
        .map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|x|PathBuf::from(x).join(".config")))?;
    Some(config.join("autostart"))
}
fn valid_filename(name:&str)->bool{
    name.ends_with(".desktop") && name.len()<=200 && name.len()>8 &&
    name.bytes().all(|c|c.is_ascii_alphanumeric() ||
        matches!(c,b'-'|b'_'|b'.'|b'+'))
    && !name.starts_with('.') && !name.contains("..")
}
fn key<'a>(contents:&'a str, wanted:&str)->Option<&'a str> {
    let mut desktop=false;
    let mut found=None;
    for line in contents.lines(){
        let line=line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            desktop=line=="[Desktop Entry]";
        } else if desktop {
            if let Some((k,v))=line.split_once('='){
                if k.trim()==wanted {found=Some(v.trim());}
            }
        }
    }
    found
}
fn valid_desktop_file(content:&str)->bool{
    content.lines().any(|line|line.trim()=="[Desktop Entry]")
        && key(content,"Type")==Some("Application")
}

/// Only user-owned regular files, never symlinks. Files may be malformed:
/// those are excluded rather than enabling an unsafe launch entry.
pub fn list_in(dir:&Path)->Vec<AutostartItem>{
    if !fs::symlink_metadata(dir).is_ok_and(|meta|meta.file_type().is_dir()){
        return Vec::new();
    }
    let Ok(entries)=fs::read_dir(dir) else {return Vec::new()};
    let mut result=Vec::new();
    for entry in entries.flatten().take(160) {
        let name=entry.file_name().to_string_lossy().to_string();
        if !valid_filename(&name) || !entry.file_type().is_ok_and(|ty|ty.is_file()) {
            continue;
        }
        let Ok(content)=fs::read_to_string(entry.path()) else {continue};
        if !valid_desktop_file(&content) {continue;}
        let title=key(&content,"Name").unwrap_or(&name).chars().take(90).collect();
        let hidden=key(&content,"Hidden")==Some("true");
        let x_disabled=key(&content,"X-GNOME-Autostart-enabled")==Some("false");
        result.push(AutostartItem{file_name:name,name:title,enabled:!hidden&&!x_disabled});
    }
    result.sort_by_key(|i|i.name.to_lowercase());
    result
}

fn rewrite(content:&str,enabled:bool)->io::Result<String> {
    if !valid_desktop_file(content){
        return Err(io::Error::new(io::ErrorKind::InvalidData,"Desktop Entry non valida"));
    }
    let mut out=String::new();
    let mut in_desktop=false;
    let mut appended=false;
    let replacement=format!("Hidden={}\nX-GNOME-Autostart-enabled={}\n",
        !enabled,enabled);
    for line in content.lines() {
        let trimmed=line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_desktop && !appended {out.push_str(&replacement);appended=true;}
            in_desktop=trimmed=="[Desktop Entry]";
        }
        if in_desktop && (trimmed.starts_with("Hidden=") ||
            trimmed.starts_with("X-GNOME-Autostart-enabled=")){
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if in_desktop && !appended {out.push_str(&replacement);}
    Ok(out)
}

pub fn set_enabled_in(dir:&Path,file_name:&str,enabled:bool)->io::Result<()>{
    if !valid_filename(file_name) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"ID non valido"));
    }
    // Prevent symlinked autostart directories or desktop files redirecting
    // the write outside the user-managed XDG directory.
    if !fs::symlink_metadata(dir)?.file_type().is_dir(){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"Directory collegata"));
    }
    let path=dir.join(file_name);
    if !fs::symlink_metadata(&path)?.file_type().is_file(){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"File non regolare"));
    }
    let previous=fs::read_to_string(&path)?;
    let updated=rewrite(&previous,enabled)?;
    let temp=dir.join(format!(".melania-autostart-{}.tmp",std::process::id()));
    let mut file=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp)?;
    let result=(||{
        file.write_all(updated.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp,&path)
    })();
    if result.is_err(){let _=fs::remove_file(temp);}
    result
}
fn make_label(text:&str,class:&str)->Label{
    let label=Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}
pub fn session_page()->GtkBox {
    let body=GtkBox::new(Orientation::Vertical,12);
    body.add_css_class("settings-body");
    body.append(&make_label("Sessione e avvio","settings-heading"));
    let Some(dir)=autostart_dir() else {
        body.append(&make_label("Directory utente XDG non disponibile.","settings-row-description"));
        return body;
    };
    let entries=list_in(&dir);
    if entries.is_empty(){
        body.append(&make_label(
            "Nessuna applicazione di avvio automatico configurata nella directory personale.",
            "settings-row-description"));
        return body;
    }
    for item in entries {
        let row=GtkBox::new(Orientation::Horizontal,11);
        row.add_css_class("settings-card");
        let words=GtkBox::new(Orientation::Vertical,3);
        words.set_hexpand(true);
        words.append(&make_label(&item.name,"settings-row-title"));
        words.append(&make_label(&item.file_name,"settings-row-description"));
        let feedback=make_label("","settings-row-description");
        words.append(&feedback);
        row.append(&words);
        let toggle=Switch::builder().active(item.enabled).build();
        toggle.set_valign(gtk::Align::Center);
        let in_transition=Rc::new(Cell::new(false));
        let id=item.file_name.clone();
        let directory=dir.clone();
        let guard=in_transition.clone();
        toggle.connect_active_notify(move |sw|{
            if guard.get() {return;}
            match set_enabled_in(&directory,&id,sw.is_active()) {
                Ok(())=>feedback.set_text(if sw.is_active(){
                    "Attiva per il prossimo accesso."
                }else{"Disattiva per il prossimo accesso."}),
                Err(error)=>{
                    feedback.set_text(&format!("Impossibile salvare: {error}"));
                    guard.set(true);
                    sw.set_active(!sw.is_active());
                    guard.set(false);
                }
            }
        });
        row.append(&toggle);
        body.append(&row);
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserve_other_desktop_keys_and_reject_malformed_paths(){
        assert!(!valid_filename("../bad.desktop"));
        assert!(!valid_filename("a/b.desktop"));
        assert!(!valid_filename(".hidden.desktop"));
        let original="[Desktop Entry]\nType=Application\nName=Demo\nExec=/bin/true\n[Extra]\nKey=ok\n";
        let off=rewrite(original,false).unwrap();
        assert_eq!(key(&off,"Hidden"),Some("true"));
        assert_eq!(key(&off,"X-GNOME-Autostart-enabled"),Some("false"));
        assert!(off.contains("Exec=/bin/true"));
        assert!(off.contains("[Extra]\nKey=ok"));
        let on=rewrite(&off,true).unwrap();
        assert_eq!(key(&on,"Hidden"),Some("false"));
        assert_eq!(key(&on,"X-GNOME-Autostart-enabled"),Some("true"));
        assert_eq!(on.matches("Hidden=").count(),1);
    }
    #[test]
    fn user_autostart_toggle_is_atomic_and_never_follows_symlinks(){
        let dir=std::env::temp_dir().join(format!("melania-autostart-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("good.desktop");
        fs::write(&path,"[Desktop Entry]\nType=Application\nName=Test\nExec=/bin/true\n").unwrap();
        let entries=list_in(&dir);
        assert_eq!(entries.len(),1);
        assert!(entries[0].enabled);
        set_enabled_in(&dir,"good.desktop",false).unwrap();
        assert!(!list_in(&dir)[0].enabled);
        set_enabled_in(&dir,"good.desktop",true).unwrap();
        assert!(list_in(&dir)[0].enabled);
        let link=dir.join("unsafe.desktop");
        std::os::unix::fs::symlink(&path,&link).unwrap();
        assert!(set_enabled_in(&dir,"unsafe.desktop",false).is_err());
        assert_eq!(list_in(&dir).len(),1);
        fs::remove_dir_all(dir).unwrap();
    }
}
