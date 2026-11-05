//! Manage per-user XDG autostart entries, without running Exec lines.
//!
//! The system-wide /etc/xdg/autostart layer and third-party startup daemons
//! are *not* altered. Only existing regular ~/.config/autostart/*.desktop
//! user files can be enabled/disabled. Changes are explicit and atomic.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Button, DropDown, Label, Orientation, Switch};
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

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct InstalledApp { pub file_name:String, pub name:String }

/// Read only desktop launchers from the standard application directories.
/// The user chooses an installed launcher: Melania never accepts arbitrary
/// shell commands or writes to the system-wide autostart directory.
fn applications_dirs()->Vec<PathBuf>{
    let mut roots=Vec::new();
    if let Some(data)=std::env::var_os("XDG_DATA_HOME").filter(|s|!s.is_empty()){
        roots.push(PathBuf::from(data).join("applications"));
    }else if let Some(home)=std::env::var_os("HOME"){
        roots.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let data_dirs=std::env::var_os("XDG_DATA_DIRS")
        .unwrap_or_else(||"/usr/local/share:/usr/share".into());
    for root in std::env::split_paths(&data_dirs) {
        if root.is_absolute() { roots.push(root.join("applications")); }
    }
    roots
}
pub fn installed_in(roots:&[PathBuf])->Vec<InstalledApp>{
    let mut apps=Vec::new();
    let mut seen=std::collections::HashSet::new();
    for root in roots {
        if !fs::symlink_metadata(root).is_ok_and(|m|m.file_type().is_dir()){continue;}
        let Ok(entries)=fs::read_dir(root) else {continue;};
        for entry in entries.flatten().take(1200) {
            let file_name=entry.file_name().to_string_lossy().to_string();
            if !valid_filename(&file_name) ||
                !entry.file_type().is_ok_and(|t|t.is_file()) ||
                !seen.insert(file_name.clone()){continue;}
            let Ok(content)=fs::read_to_string(entry.path()) else {continue;};
            if !valid_desktop_file(&content) || key(&content,"Exec").is_none()
                || key(&content,"Hidden")==Some("true")
                || key(&content,"NoDisplay")==Some("true"){continue;}
            apps.push(InstalledApp{
                name:key(&content,"Name").unwrap_or(&file_name).chars().take(90).collect(),
                file_name
            });
        }
    }
    apps.sort_by_key(|a|a.name.to_lowercase());
    apps
}
/// Copy the selected installed desktop entry into the *user's* XDG autostart.
/// Resolve only a validated basename in a regular application directory.
/// Refuse overwrites so user customizations and explicit opt-outs survive.
pub fn add_installed_in(dir:&Path,roots:&[PathBuf],file_name:&str)->io::Result<()>{
    if !valid_filename(file_name){
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"ID applicazione non valido"));
    }
    let source=roots.iter().filter_map(|root|{
        if !fs::symlink_metadata(root).is_ok_and(|m|m.file_type().is_dir()){return None;}
        let path=root.join(file_name);
        if !fs::symlink_metadata(&path).is_ok_and(|m|m.file_type().is_file()){return None;}
        let data=fs::read_to_string(path).ok()?;
        if valid_desktop_file(&data) && key(&data,"Exec").is_some()
            && key(&data,"Hidden")!=Some("true") && key(&data,"NoDisplay")!=Some("true"){
            Some(data)
        } else {None}
    }).next().ok_or_else(||io::Error::new(io::ErrorKind::NotFound,"Applicazione non installata"))?;
    // Never follow a linked autostart directory.
    if !dir.exists(){
        let parent=dir.parent().ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"Percorso errato"))?;
        if !fs::symlink_metadata(parent)?.file_type().is_dir(){
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,"Directory di configurazione collegata"));
        }
        fs::create_dir(dir)?;
    }
    if !fs::symlink_metadata(dir)?.file_type().is_dir(){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"Directory di autostart collegata"));
    }
    let dest=dir.join(file_name);
    let content=rewrite(&source,true)?;
    // create_new avoids overwriting another user's choice or a symlink.
    let mut file=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&dest)?;
    let result=(||{file.write_all(content.as_bytes())?;file.sync_all()})();
    if result.is_err(){let _=fs::remove_file(dest);}
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
    let roots=applications_dirs();
    let installed=installed_in(&roots);
    body.append(&make_label("Aggiungi un'applicazione installata","settings-section-heading"));
    if installed.is_empty(){
        body.append(&make_label("Nessun launcher XDG installato disponibile.","settings-row-description"));
    } else {
        let names=installed.iter().map(|a|a.name.as_str()).collect::<Vec<_>>();
        let choice=DropDown::from_strings(&names);
        choice.set_hexpand(true);
        let add=Button::with_label("Aggiungi all'avvio");
        add.add_css_class("settings-save");
        let controls=GtkBox::new(Orientation::Horizontal,8);
        controls.append(&choice);
        controls.append(&add);
        body.append(&controls);
        let feedback=make_label("Le modifiche saranno attive al prossimo accesso.","settings-row-description");
        body.append(&feedback);
        let selected_dir=dir.clone();
        add.connect_clicked(move |_| {
            let Some(app)=installed.get(choice.selected() as usize) else {return;};
            match add_installed_in(&selected_dir,&roots,&app.file_name){
                Ok(())=>{
                    feedback.set_text(&format!("{} aggiunta. Riapri questa pagina per gestire il nuovo interruttore.",app.name));
                }
                Err(e)=>feedback.set_text(&format!("Impossibile aggiungere {}: {e}",app.name))
            }
        });
    }
    body.append(&make_label("Applicazioni configurate","settings-section-heading"));
    let entries=list_in(&dir);
    if entries.is_empty(){
        body.append(&make_label(
            "Nessuna applicazione di avvio automatico configurata nella directory personale.",
            "settings-row-description"));
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
    fn adding_installed_launcher_preserves_exec_and_never_overwrites() {
        let base=std::env::temp_dir().join(format!("melania-add-app-{}",std::process::id()));
        let apps=base.join("applications");
        let config=base.join("config");
        let autostart=config.join("autostart");
        fs::create_dir_all(&apps).unwrap();
        fs::create_dir_all(&config).unwrap();
        let launcher=apps.join("sample.desktop");
        fs::write(&launcher,"[Desktop Entry]\nType=Application\nName=Sample\nExec=/usr/bin/sample --arg\nHidden=false\n").unwrap();
        let roots=vec![apps.clone()];
        assert_eq!(installed_in(&roots)[0].file_name,"sample.desktop");
        add_installed_in(&autostart,&roots,"sample.desktop").unwrap();
        assert!(fs::read_to_string(autostart.join("sample.desktop")).unwrap()
            .contains("Exec=/usr/bin/sample --arg"));
        assert!(add_installed_in(&autostart,&roots,"sample.desktop").is_err());
        assert!(add_installed_in(&autostart,&roots,"../bad.desktop").is_err());
        fs::write(apps.join("bad.desktop"),"[Desktop Entry]\nType=Application\nName=Bad\n").unwrap();
        assert!(add_installed_in(&autostart,&roots,"bad.desktop").is_err());
        let external=base.join("external");
        fs::create_dir_all(&external).unwrap();
        fs::remove_dir_all(&autostart).unwrap();
        std::os::unix::fs::symlink(&external,&autostart).unwrap();
        assert!(add_installed_in(&autostart,&roots,"sample.desktop").is_err());
        fs::remove_dir_all(base).unwrap();
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
