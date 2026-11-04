//! Launch the real XDG Autostart entries for a standalone Melania session.
//!
//! GIO parses Exec, quoting, field codes, TryExec and startup metadata.
//! Never exec() a desktop file through a shell. Only enable in an independent
//! Melania login after the Wayland compositor has become available.
use gtk::gio::{self,prelude::*};
use std::{collections::BTreeMap,env,fs,path::{Path,PathBuf},process::ExitCode};

fn eligible(get:impl Fn(&str)->Option<String>)->bool{
    get("MELANIA_EXTERNAL_COMPOSITOR").as_deref()==Some("labwc")
        && get("MELANIA_NESTED_SESSION").as_deref()!=Some("1")
        && get("MELANIA_SMOKE_OPEN_START").as_deref()!=Some("1")
        && get("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()==Some("1")
        && get("WAYLAND_DISPLAY").is_some_and(|w|!w.is_empty())
}

/// Priority: /etc/xdg/autostart < XDG_CONFIG_DIRS in declared order
/// < XDG_CONFIG_HOME. The first configured system dir is higher priority.
fn dirs(get:impl Fn(&str)->Option<String>)->Vec<PathBuf>{
    let defaults=get("XDG_CONFIG_DIRS").filter(|s|!s.is_empty())
        .unwrap_or_else(||"/etc/xdg".to_owned());
    let mut system=defaults.split(':')
        .filter(|s|s.starts_with('/'))
        .map(|s|Path::new(s).join("autostart"))
        .collect::<Vec<_>>();
    system.reverse();
    let user=get("XDG_CONFIG_HOME").filter(|p|p.starts_with('/'))
        .map(PathBuf::from)
        .or_else(||get("HOME").map(|h|PathBuf::from(h).join(".config")));
    if let Some(dir)=user{system.push(dir.join("autostart"));}
    system
}

fn valid_id(name:&str)->bool{
    name.ends_with(".desktop") && name.len()<=200 && name.len()>8
        && !name.starts_with('.') && !name.contains("..")
        && name.bytes().all(|c|c.is_ascii_alphanumeric()
            || matches!(c,b'_'|b'-'|b'.'|b'+'))
}

/// Merge only by basename: a user Hidden=true entry suppresses the
/// system entry with the same desktop ID, rather than launching both.
fn enumerate(paths:&[PathBuf])->BTreeMap<String,PathBuf>{
    let mut files=BTreeMap::new();
    for dir in paths{
        let Ok(entries)=fs::read_dir(dir) else{continue};
        for item in entries.flatten().take(512){
            let name=item.file_name().to_string_lossy().to_string();
            if !valid_id(&name) || !item.file_type().is_ok_and(|t|t.is_file()){
                continue;
            }
            files.insert(name,item.path());
        }
    }
    files
}

fn field<'a>(source:&'a str,key:&str)->Option<&'a str>{
    let mut inside=false;
    for line in source.lines(){
        let entry=line.trim();
        if entry.starts_with('[') && entry.ends_with(']'){
            inside=entry=="[Desktop Entry]";
            continue;
        }
        if !inside || entry.starts_with('#'){continue;}
        if let Some((k,value))=entry.split_once('='){
            if k.trim()==key{return Some(value.trim());}
        }
    }
    None
}

fn matches_desktop(field_value:Option<&str>,desktops:&[&str])->bool{
    field_value.is_some_and(|list|list.split(';')
        .filter(|s|!s.is_empty())
        .any(|s|desktops.iter().any(|d|d.eq_ignore_ascii_case(s))))
}

fn should_launch(source:&str,desktops:&[&str])->bool{
    if field(source,"Type")!=Some("Application")
        || field(source,"Exec").is_none_or(str::is_empty)
        || field(source,"Hidden")==Some("true")
        || field(source,"X-GNOME-Autostart-enabled")==Some("false")
        || field(source,"X-MATE-Autostart-enabled")==Some("false")
        || field(source,"X-XFCE-Autostart-enabled")==Some("false") {
        return false;
    }
    if field(source,"OnlyShowIn").is_some()
        && !matches_desktop(field(source,"OnlyShowIn"),desktops){return false;}
    if matches_desktop(field(source,"NotShowIn"),desktops){return false;}
    true
}

fn launch_all()->Result<(),String>{
    if !eligible(|k|env::var(k).ok()){
        return Err("autostart requires independent Melania login; nested host untouched".into());
    }
    let dirs=dirs(|k|env::var(k).ok());
    let candidates=enumerate(&dirs);
    let desktop_var=env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_|"Melania:labwc".into());
    let desktops=desktop_var.split(':').collect::<Vec<_>>();
    let mut started=0;
    let mut failed=0;
    for (id,path) in candidates{
        let Ok(meta)=fs::metadata(&path) else{continue};
        if meta.len()>262_144 {continue;}
        let Ok(content)=fs::read_to_string(&path) else{continue};
        if !should_launch(&content,&desktops){continue;}
        let Some(info)=gio::DesktopAppInfo::from_filename(&path) else{
            eprintln!("MELANIA_AUTOSTART_INVALID={id}");
            failed+=1;continue;
        };
        if let Err(error)=info.launch(&[],None::<&gio::AppLaunchContext>){
            eprintln!("MELANIA_AUTOSTART_LAUNCH_ERROR={id}: {error}");
            failed+=1;
        }else{started+=1;}
    }
    println!("MELANIA_AUTOSTART_LAUNCHED={started} FAILED={failed}");
    Ok(())
}

fn main()->ExitCode{
    if env::args().count()!=1{
        eprintln!("MELANIA_AUTOSTART_BLOCKED=no external command arguments accepted");
        return ExitCode::from(78);
    }
    match launch_all(){
        Ok(())=>ExitCode::SUCCESS,
        Err(error)=>{
            eprintln!("MELANIA_AUTOSTART_BLOCKED={error}");
            ExitCode::from(78)
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn env_map(key:&str)->Option<String>{
        match key{
            "MELANIA_EXTERNAL_COMPOSITOR"=>Some("labwc".into()),
            "MELANIA_ALLOW_HOST_POWER_ACTIONS"=>Some("1".into()),
            "WAYLAND_DISPLAY"=>Some("wayland-0".into()),
            _=>None
        }
    }
    #[test]
    fn previews_do_not_launch_other_desktops_autostart(){
        assert!(eligible(env_map));
        assert!(!eligible(|k|if k=="MELANIA_NESTED_SESSION"{Some("1".into())}else{env_map(k)}));
        assert!(!eligible(|k|if k=="WAYLAND_DISPLAY"{None}else{env_map(k)}));
        assert!(!eligible(|k|if k=="MELANIA_EXTERNAL_COMPOSITOR"{Some("cinnamon".into())}else{env_map(k)}));
    }
    #[test]
    fn user_hidden_overrides_system_and_no_shell_interpretation(){
        let root=env::temp_dir().join(format!("melania-autostart-parity-{}",std::process::id()));
        let sys=root.join("system/autostart");
        let user=root.join("user/autostart");
        fs::create_dir_all(&sys).unwrap();fs::create_dir_all(&user).unwrap();
        fs::write(sys.join("demo.desktop"),"[Desktop Entry]\nType=Application\nExec=demo\n").unwrap();
        fs::write(user.join("demo.desktop"),"[Desktop Entry]\nType=Application\nExec=demo\nHidden=true\n").unwrap();
        fs::write(user.join("x.desktop"),"[Desktop Entry]\nType=Application\nExec=demo\n").unwrap();
        let items=enumerate(&[sys,user.clone()]);
        assert_eq!(items.len(),2);
        let chosen=fs::read_to_string(items.get("demo.desktop").unwrap()).unwrap();
        assert!(!should_launch(&chosen,&["Melania","labwc"]));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn only_show_in_and_not_show_in_are_honored(){
        let base="[Desktop Entry]\nType=Application\nExec=/usr/bin/example\n";
        assert!(should_launch(base,&["Melania","labwc"]));
        assert!(should_launch(&(base.to_owned()+"OnlyShowIn=XFCE;Melania;\n"),&["Melania","labwc"]));
        assert!(!should_launch(&(base.to_owned()+"OnlyShowIn=XFCE;\n"),&["Melania","labwc"]));
        assert!(!should_launch(&(base.to_owned()+"NotShowIn=Melania;\n"),&["Melania","labwc"]));
        assert!(!should_launch(&(base.to_owned()+"X-GNOME-Autostart-enabled=false\n"),&["Melania","labwc"]));
        assert!(!should_launch(&(base.to_owned()+"Hidden=true\n"),&["Melania","labwc"]));
        assert!(!valid_id("../evil.desktop"));
        assert!(!valid_id(".hidden.desktop"));
    }
}
