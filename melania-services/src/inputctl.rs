//! User-scoped, atomic labwc libinput settings for standalone Melania.
//!
//! Never touch ~/.config/labwc, xfconf, libinput udev rules or Cinnamon.
//! Only four predefined option keys and bounded values are accepted; no
//! arbitrary XML, shell command, executable name or filesystem path enters.
use std::{env,fs,io::{self,Write},os::unix::fs::{DirBuilderExt,OpenOptionsExt,PermissionsExt},
    path::{Path,PathBuf},process::{Command,ExitCode}};

const TEMPLATE:&str=include_str!("../../integration/labwc/rc.xml");
const CONFIG_HEADER:&str="<!-- GENERATED-MELANIA-LABWC-INPUT-V1; edit Settings, not this file -->";
const END:&str="</labwc_config>";
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct InputPrefs{tap:bool,natural_scroll:bool,left_handed:bool,speed:i32}
impl Default for InputPrefs{
    fn default()->Self{Self{tap:true,natural_scroll:false,left_handed:false,speed:0}}
}
impl InputPrefs{
    fn parse(text:&str)->Result<Self,String>{
        if text.len()>1024 {return Err("preferences too large".into());}
        let mut result=Self::default();
        for line in text.lines(){
            let (key,value)=line.split_once('=').ok_or("invalid line")?;
            match key{
                "tap"=>result.tap=parse_bool(value)?,
                "natural_scroll"=>result.natural_scroll=parse_bool(value)?,
                "left_handed"=>result.left_handed=parse_bool(value)?,
                "speed"=>result.speed=parse_speed(value)?,
                _=>return Err("unknown input preference".into()),
            }
        }
        Ok(result)
    }
    fn encoded(self)->String{
        format!("tap={}\nnatural_scroll={}\nleft_handed={}\nspeed={}\n",
            if self.tap{"yes"}else{"no"},
            if self.natural_scroll{"yes"}else{"no"},
            if self.left_handed{"yes"}else{"no"},self.speed)
    }
    fn update(&mut self,key:&str,value:&str)->Result<(),String>{
        match key{
            "tap"=>self.tap=parse_bool(value)?,
            "natural_scroll"=>self.natural_scroll=parse_bool(value)?,
            "left_handed"=>self.left_handed=parse_bool(value)?,
            "speed"=>self.speed=parse_speed(value)?,
            _=>return Err("invalid settings key".into()),
        }
        Ok(())
    }
}
fn parse_bool(s:&str)->Result<bool,String>{
    match s{"yes"=>Ok(true),"no"=>Ok(false),_=>Err("expected yes or no".into())}
}
fn parse_speed(s:&str)->Result<i32,String>{
    let n=s.parse::<i32>().map_err(|_|"invalid pointer speed")?;
    if !(-100..=100).contains(&n){return Err("speed range must be -100..100".into());}
    Ok(n)
}

const SHORTCUT_ACTIONS:[&str;5]=["start","files","settings","lock","terminal"];
const SHORTCUT_DEFAULTS:[&str;5]=["W-space","W-e","W-i","W-l","W-Return"];
// A bounded set of labwc key syntax. No arbitrary XML or command strings.
const SHORTCUT_CHOICES:[&str;12]=[
    "W-space","W-e","W-i","W-l","W-Return","W-f","W-k","W-d",
    "W-t","W-p","C-A-e","C-A-t"
];
#[derive(Clone,Debug,PartialEq,Eq)]
struct ShortcutPrefs{keys:[String;5]}
impl Default for ShortcutPrefs{
    fn default()->Self{
        Self{keys:SHORTCUT_DEFAULTS.map(str::to_owned)}
    }
}
impl ShortcutPrefs{
    fn parse(source:&str)->Result<Self,String>{
        if source.len()>1024{return Err("shortcuts too large".into());}
        let mut result=Self::default();
        for line in source.lines(){
            let (key,value)=line.split_once('=').ok_or("invalid shortcut preference")?;
            let Some(index)=SHORTCUT_ACTIONS.iter().position(|v|*v==key) else{
                return Err("unknown shortcut action".into());
            };
            result.keys[index]=value.to_owned();
        }
        result.validate()?;
        Ok(result)
    }
    fn from_values(values:&[String])->Result<Self,String>{
        if values.len()!=SHORTCUT_ACTIONS.len(){
            return Err("exactly five shortcuts required".into());
        }
        let keys: [String;5]=values.to_vec().try_into()
            .map_err(|_|"invalid shortcuts array")?;
        let result=Self{keys};
        result.validate()?;
        Ok(result)
    }
    fn validate(&self)->Result<(),String>{
        let mut unique=std::collections::HashSet::new();
        for value in &self.keys{
            if !SHORTCUT_CHOICES.contains(&value.as_str()) ||
                !unique.insert(value.as_str()){
                return Err("shortcut unavailable or duplicated".into());
            }
        }
        Ok(())
    }
    fn encoded(&self)->String{
        SHORTCUT_ACTIONS.iter().zip(self.keys.iter())
            .map(|(name,value)|format!("{name}={value}\n")).collect()
    }
}
fn read_shortcuts(path:&Path)->Result<ShortcutPrefs,String>{
    match fs::symlink_metadata(path){
        Ok(meta) if meta.file_type().is_file() => {
            ShortcutPrefs::parse(&fs::read_to_string(path).map_err(|e|e.to_string())?)
        },
        Ok(_) => Err("shortcuts preference file is not regular".into()),
        Err(error) if error.kind()==io::ErrorKind::NotFound => Ok(ShortcutPrefs::default()),
        Err(error) => Err(error.to_string()),
    }
}
fn render_shortcuts(source:&str,shortcuts:&ShortcutPrefs)->Result<String,String>{
    shortcuts.validate()?;
    let mut xml=source.to_owned();
    for (index,original) in SHORTCUT_DEFAULTS.iter().enumerate(){
        let needle=format!("<keybind key=\"{original}\">");
        if xml.matches(&needle).count()!=1{
            return Err(format!("missing expected shortcut {original}"));
        }
        xml=xml.replacen(&needle,&format!("<keybind key=\"MELANIA_SLOT_{index}\">"),1);
    }
    for (index,configured) in shortcuts.keys.iter().enumerate(){
        xml=xml.replace(&format!("MELANIA_SLOT_{index}"),configured);
    }
    Ok(xml)
}

fn config_home()->Result<PathBuf,String>{
    let base=env::var_os("XDG_CONFIG_HOME").filter(|x|!x.is_empty())
        .map(PathBuf::from).or_else(||env::var_os("HOME")
            .filter(|x|!x.is_empty()).map(|home|PathBuf::from(home).join(".config")))
        .ok_or("missing XDG_CONFIG_HOME and HOME")?;
    if !base.is_absolute(){return Err("config home must be absolute".into());}
    Ok(base.join("melania"))
}
fn ensure_dir(path:&Path)->Result<(),String>{
    if path.is_symlink(){return Err("symlink config directory is forbidden".into());}
    if !path.exists(){
        fs::DirBuilder::new().recursive(true).mode(0o700).create(path)
            .map_err(|e|format!("cannot create config directory: {e}"))?;
    }
    if !fs::symlink_metadata(path).is_ok_and(|m|
        m.file_type().is_dir()&&m.permissions().mode()&0o077==0)
    {return Err("config directory must be private (0700)".into());}
    Ok(())
}
fn write_atomic(path:&Path,contents:&str)->Result<(),String>{
    let parent=path.parent().ok_or("missing parent path")?;
    ensure_dir(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()){
        return Err("refuse symlink or non-regular config file".into());
    }
    let temp=parent.join(format!(".melania-inputctl-{}.tmp",std::process::id()));
    let mut file=fs::OpenOptions::new().create_new(true).write(true).mode(0o600)
        .open(&temp).map_err(|e|format!("cannot write temporary config: {e}"))?;
    let result=(||->io::Result<()>{
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp,path)
    })();
    if result.is_err(){let _=fs::remove_file(&temp);}
    result.map_err(|e|format!("cannot save settings: {e}"))
}
fn load(p:&Path)->Result<InputPrefs,String>{
    if !p.exists(){return Ok(InputPrefs::default());}
    if fs::symlink_metadata(p).is_ok_and(|m|!m.file_type().is_file()){
        return Err("unsafe preferences file".into());
    }
    let text=fs::read_to_string(p).map_err(|e|e.to_string())?;
    InputPrefs::parse(&text)
}
fn render(p:InputPrefs,shortcuts:&ShortcutPrefs)->Result<String,String>{
    if TEMPLATE.matches(END).count()!=1{return Err("unexpected labwc template".into());}
    // All interpolated values are numeric or exact yes/no constants.
    let value=|on|if on{"yes"}else{"no"};
    let block=format!(r#"
  <libinput>
    <device category="touchpad">
      <tap>{}</tap>
      <naturalScroll>{}</naturalScroll>
      <disableWhileTyping>yes</disableWhileTyping>
    </device>
    <device category="non-touch">
      <leftHanded>{}</leftHanded>
      <pointerSpeed>{:.2}</pointerSpeed>
    </device>
  </libinput>
"#,value(p.tap),value(p.natural_scroll),value(p.left_handed),p.speed as f64/100.0);
    let xml=render_shortcuts(TEMPLATE,shortcuts)?;
    Ok(format!("{CONFIG_HEADER}\n{}",xml.replace(END,&format!("{block}{END}"))))
}
fn session_guard()->Result<(),String>{
    if env::var("MELANIA_EXTERNAL_COMPOSITOR").as_deref()!=Ok("labwc")
        || env::var("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()!=Ok("1")
        || env::var("MELANIA_NESTED_SESSION").as_deref()==Ok("1")
        || env::var("MELANIA_SMOKE_OPEN_START").as_deref()==Ok("1")
    {return Err("input changes are disabled outside standalone Melania login".into());}
    Ok(())
}
fn generated_path(base:&Path)->PathBuf{base.join("labwc").join("rc.xml")}
fn prepare(p:InputPrefs,shortcuts:&ShortcutPrefs,base:&Path)->Result<(),String>{
    let out=generated_path(base);
    if out.exists() {
        let previous=fs::read_to_string(&out).map_err(|e|e.to_string())?;
        if !previous.starts_with(CONFIG_HEADER){
            return Err("refuse to overwrite manually edited labwc config".into());
        }
    }
    write_atomic(&out,&render(p,shortcuts)?)?;
    println!("MELANIA_LABWC_INPUT_CONFIG={}",out.display());
    Ok(())
}
fn commit_and_reload(prefs:InputPrefs,shortcuts:&ShortcutPrefs,base:&Path,prefs_path:&Path)->Result<(),String>{
    write_atomic(prefs_path,&prefs.encoded())?;
    prepare(prefs,shortcuts,base)?;
    let status=Command::new("labwc").arg("--reconfigure").status()
        .map_err(|e|format!("cannot reload labwc: {e}"))?;
    if !status.success(){return Err("labwc did not reload input settings".into());}
    println!("MELANIA_LABWC_INPUT_RECONFIGURED=true");
    Ok(())
}
fn run()->Result<(),String>{
    let args=env::args().skip(1).collect::<Vec<_>>();
    let base=config_home()?;
    let prefs_path=base.join("input-prefs");
    let shortcuts_path=base.join("shortcut-prefs");
    let shortcuts=read_shortcuts(&shortcuts_path)?;
    let mut prefs=load(&prefs_path)?;
    match args.as_slice(){
        [command] if command=="show"=>{
            print!("{}",prefs.encoded());Ok(())
        },
        [command] if command=="prepare"=>{
            session_guard()?;prepare(prefs,&shortcuts,&base)
        },
        [command,key,value] if command=="set"=>{
            session_guard()?;
            if env::var("WAYLAND_DISPLAY").is_err(){return Err("no Wayland session".into());}
            prefs.update(key,value)?;
            commit_and_reload(prefs,&shortcuts,&base,&prefs_path)
        },
        [command,tap,natural,left,speed] if command=="apply"=>{
            session_guard()?;
            if env::var("WAYLAND_DISPLAY").is_err(){return Err("no Wayland session".into());}
            prefs.update("tap",tap)?;
            prefs.update("natural_scroll",natural)?;
            prefs.update("left_handed",left)?;
            prefs.update("speed",speed)?;
            commit_and_reload(prefs,&base,&prefs_path)
        },

        [command] if command=="shortcuts-show"=>{
            print!("{}",shortcuts.encoded());Ok(())
        },
        [command,one,two,three,four,five] if command=="shortcuts-apply"=>{
            session_guard()?;
            if env::var("WAYLAND_DISPLAY").is_err(){return Err("no Wayland session".into());}
            let chosen=ShortcutPrefs::from_values(
                &[one.clone(),two.clone(),three.clone(),four.clone(),five.clone()])?;
            // Check the template BEFORE persisting user preferences.
            render(prefs,&chosen)?;
            write_atomic(&shortcuts_path,&chosen.encoded())?;
            prepare(prefs,&chosen,&base)?;
            let status=Command::new("labwc").arg("--reconfigure").status()
                .map_err(|e|format!("cannot reload labwc shortcuts: {e}"))?;
            if !status.success(){return Err("labwc did not reload shortcuts".into());}
            println!("MELANIA_LABWC_SHORTCUTS_RECONFIGURED=true");
            Ok(())
        },
        _=>Err("usage: melania-inputctl show|prepare|set <key> <value>|apply <tap> <scroll> <left> <speed>|shortcuts-show|shortcuts-apply <start> <files> <settings> <lock> <terminal>".into()),
    }
}
fn main()->ExitCode{
    match run(){
        Ok(())=>ExitCode::SUCCESS,
        Err(e)=>{eprintln!("MELANIA_LABWC_INPUT_BLOCKED={e}");ExitCode::from(78)}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn saved_input_prefs_roundtrip_strict_and_bounded(){
        let mut p=InputPrefs::default();
        for (key,value) in [
            ("tap","no"),("natural_scroll","yes"),("left_handed","yes"),("speed","-42")
        ]{p.update(key,value).unwrap();}
        assert_eq!(InputPrefs::parse(&p.encoded()).unwrap(),p);
        assert!(p.update("speed","101").is_err());
        assert!(p.update("speed","-101").is_err());
        assert!(p.update("tap","true").is_err());
        assert!(p.update("evil-key","yes").is_err());
    }
    #[test]
    fn shortcuts_are_unique_whitelisted_and_replace_every_original_key(){
        let mut s=ShortcutPrefs::default();
        s.keys[0]="W-p".into();
        s.keys[1]="W-f".into();
        assert!(s.validate().is_ok());
        let parsed=ShortcutPrefs::parse(&s.encoded()).unwrap();
        assert_eq!(parsed,s);
        let out=render(InputPrefs::default(),&s).unwrap();
        assert!(out.contains(r#"<keybind key="W-p">"#));
        assert!(out.contains(r#"<keybind key="W-f">"#));
        assert!(!out.contains("MELANIA_SLOT_"));
        let mut duplicate=s.clone();
        duplicate.keys[2]="W-f".into();
        assert!(duplicate.validate().is_err());
        assert!(ShortcutPrefs::parse("start=Alt-F4\n").is_err());
        assert!(ShortcutPrefs::parse("start=W-p\nunknown=W-i\n").is_err());
        assert!(ShortcutPrefs::parse("start=W-p\nstart=W-p\n").is_ok());
    }
    #[test]fn generated_xml_is_valid_labwc_input_and_contains_no_shell(){
        let value=render(InputPrefs::default(),&ShortcutPrefs::default()).unwrap();
        assert!(value.starts_with(CONFIG_HEADER));
        assert!(value.contains("<device category=\"touchpad\">"));
        assert!(value.contains("<tap>yes</tap>"));
        assert!(value.contains("<device category=\"non-touch\">"));
        assert!(value.contains("<pointerSpeed>0.00</pointerSpeed>"));
        assert_eq!(value.matches("</labwc_config>").count(),1);
    }
}
