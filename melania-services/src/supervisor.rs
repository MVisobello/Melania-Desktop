//! Session-local, fail-closed process supervisor for Melania's proven labwc backend.
//!
//! The experimental native compositor remains separate. This service does
//! not become a display manager, acquire DRM devices or run as root.
//!
//! One transient panel/desktop/control crash must not log the user out.
//! Repeated crashes are bounded; the lock/idle guard never restarts silently.
//! On exhaustion, terminate ONLY the validated owned labwc session.
use std::{
    collections::VecDeque,
    env, fs, io,
    os::unix::{fs::{FileTypeExt,MetadataExt,PermissionsExt},process::CommandExt},
    path::{Path,PathBuf},
    process::{Child,Command,ExitCode,Stdio},
    sync::atomic::{AtomicBool,Ordering},
    thread,
    time::{Duration,Instant},
};
use libc::{SIGINT,SIGTERM,SIGHUP,SIGKILL};

static SHUTDOWN:AtomicBool=AtomicBool::new(false);
const POLL:Duration=Duration::from_millis(180);
const WINDOW:Duration=Duration::from_secs(60);
const REST_DELAY:Duration=Duration::from_millis(650);
const MAX_RESTARTS:usize=3;

extern "C" fn signal_handler(_:i32){
    SHUTDOWN.store(true,Ordering::Relaxed);
}
fn install_signals(){
    // Signal handlers only update an atomic, never allocate or do I/O.
    unsafe {
        libc::signal(SIGINT,signal_handler as libc::sighandler_t);
        libc::signal(SIGTERM,signal_handler as libc::sighandler_t);
        libc::signal(SIGHUP,signal_handler as libc::sighandler_t);
    }
}

#[derive(Default)]
struct RestartBudget { recent:VecDeque<Instant> }
impl RestartBudget {
    fn allow(&mut self,now:Instant)->bool {
        while self.recent.front().is_some_and(|time|
            now.saturating_duration_since(*time)>=WINDOW){
            self.recent.pop_front();
        }
        if self.recent.len()>=MAX_RESTARTS{return false;}
        self.recent.push_back(now);
        true
    }
}

#[derive(Clone,Copy,PartialEq,Eq)]
enum Policy { RecoverCritical, FailClosed, RecoverOptional, Once }

struct Service {
    name:&'static str,
    program:PathBuf,
    args:Vec<String>,
    policy:Policy,
    child:Option<Child>,
    budget:RestartBudget,
    retry_after:Option<Instant>,
    disabled:bool,
}
impl Service {
    fn new(name:&'static str,program:PathBuf,args:Vec<String>,policy:Policy)->Self {
        Self{name,program,args,policy,child:None,budget:RestartBudget::default(),
            retry_after:None,disabled:false}
    }
    fn start(&mut self)->io::Result<()>{
        let mut command=Command::new(&self.program);
        command.args(&self.args).stdin(Stdio::null());
        // Dedicated process group permits cleanup of owned children without
        // killing unrelated processes in the user's XFCE/Cinnamon session.
        command.process_group(0);
        let child=command.spawn()?;
        println!("MELANIA_LABWC_SERVICE_STARTED={} pid={}",self.name,child.id());
        self.child=Some(child);
        self.retry_after=None;
        Ok(())
    }
    fn poll(&mut self,now:Instant)->Result<bool,String>{
        if self.disabled {return Ok(true);}
        if let Some(ref mut child)=self.child {
            match child.try_wait() {
                Ok(None)=>return Ok(true),
                Ok(Some(status))=>{
                    eprintln!("MELANIA_LABWC_COMPONENT_EXIT={} status={status}",self.name);
                    self.child=None;
                    if self.policy==Policy::Once {
                        self.disabled=true;
                        return Ok(true);
                    }
                    if self.policy==Policy::FailClosed {
                        return Err(format!("security guard exited: {}",self.name));
                    }
                    if !self.budget.allow(now) {
                        if self.policy==Policy::RecoverOptional {
                            eprintln!("MELANIA_LABWC_OPTIONAL_DISABLED={}",self.name);
                            self.disabled=true;
                            return Ok(true);
                        }
                        return Err(format!("critical component crash-loop: {}",self.name));
                    }
                    self.retry_after=Some(now+REST_DELAY);
                    println!("MELANIA_LABWC_RESTART_SCHEDULED={}",self.name);
                    return Ok(true);
                },
                Err(error)=>return Err(format!("cannot inspect {}: {error}",self.name)),
            }
        }
        if self.retry_after.is_some_and(|deadline|now<deadline){
            return Ok(true);
        }
        match self.start(){
            Ok(())=>{
                if self.budget.recent.len()>0 {
                    println!("MELANIA_LABWC_COMPONENT_RECOVERED={}",self.name);
                }
                Ok(true)
            },
            Err(error)=>{
                if self.policy==Policy::Once || self.policy==Policy::RecoverOptional {
                    eprintln!("MELANIA_LABWC_OPTIONAL_DISABLED={} error={error}",self.name);
                    self.disabled=true;
                    return Ok(true);
                }
                Err(format!("cannot start {}: {error}",self.name))
            }
        }
    }
    fn stop(&mut self){
        if let Some(mut child)=self.child.take(){
            let pid=child.id() as i32;
            // The child was spawned into its own process group.
            unsafe {libc::kill(-pid,SIGTERM);}
            let deadline=Instant::now()+Duration::from_millis(900);
            loop {
                match child.try_wait(){
                    Ok(Some(_))=>break,
                    Ok(None) if Instant::now()<deadline => thread::sleep(Duration::from_millis(25)),
                    _=>{
                        unsafe{libc::kill(-pid,SIGKILL);}
                        let _=child.wait();
                        break;
                    }
                }
            }
        }
    }
}

fn variable(key:&str)->Result<String,String>{
    env::var(key).map_err(|_|format!("missing environment variable {key}"))
        .and_then(|v|if v.is_empty(){Err(format!("empty {key}"))}else{Ok(v)})
}
fn labwc_owned_pid()->Result<i32,String>{
    if variable("MELANIA_EXTERNAL_COMPOSITOR")?.as_str()!="labwc" ||
       variable("XDG_CURRENT_DESKTOP")?.as_str()!="Melania:labwc" {
        return Err("not a Melania labwc session".into());
    }
    let id=variable("LABWC_PID")?.parse::<i32>()
        .map_err(|_|"invalid LABWC_PID")?;
    if id<=1{return Err("unsafe LABWC_PID".into());}
    let proc=PathBuf::from(format!("/proc/{id}"));
    let meta=fs::metadata(&proc).map_err(|e|format!("labwc process missing: {e}"))?;
    if meta.uid()!=unsafe{libc::geteuid()}{
        return Err("labwc belongs to another user".into());
    }
    if fs::read_to_string(proc.join("comm")).map_err(|e|e.to_string())?.trim()!="labwc"{
        return Err("LABWC_PID does not name labwc".into());
    }
    Ok(id)
}
fn private_runtime(socket:&Path)->Result<(),String>{
    let runtime=PathBuf::from(variable("XDG_RUNTIME_DIR")?);
    if !runtime.is_absolute() || !socket.is_absolute() ||
       socket.parent()!=Some(runtime.as_path()){
        return Err("private IPC socket outside XDG_RUNTIME_DIR".into());
    }
    let meta=fs::symlink_metadata(&runtime).map_err(|e|e.to_string())?;
    if !meta.file_type().is_dir() || meta.uid()!=unsafe{libc::geteuid()} ||
       meta.permissions().mode()&0o077!=0 {
        return Err("runtime directory not private or owned".into());
    }
    Ok(())
}
fn clean_socket(socket:&Path)->Result<(),String>{
    match fs::symlink_metadata(socket){
        Ok(meta) if meta.file_type().is_socket()=>
            fs::remove_file(socket).map_err(|e|format!("cannot unlink session IPC: {e}")),
        Ok(_)=>Err("session IPC path is not a Unix socket".into()),
        Err(error) if error.kind()==io::ErrorKind::NotFound=>Ok(()),
        Err(error)=>Err(error.to_string()),
    }
}
fn check_lock(bin:&Path)->Result<(),String>{
    let status=Command::new(bin.join("melania-sessionctl")).arg("check-lock")
        .status().map_err(|e|format!("cannot check secure lock: {e}"))?;
    if status.success(){Ok(())}else{Err("secure lock unavailable".into())}
}
fn terminate_owned_labwc(){
    if let Ok(pid)=labwc_owned_pid(){
        // Only the currently owned labwc process can be terminated.
        unsafe{libc::kill(pid,SIGTERM);}
        println!("MELANIA_LABWC_SAFE_LOGOUT_REQUESTED=true");
    }
}

fn run()->Result<(),String>{
    if unsafe{libc::geteuid()}==0 {return Err("refusing to supervise a root session".into());}
    let labwc=labwc_owned_pid()?;
    let root=PathBuf::from(variable("MELANIA_ROOT")?);
    let bin=PathBuf::from(variable("MELANIA_BIN_DIR")?);
    let socket=PathBuf::from(variable("MELANIA_CONTROL_SOCKET")?);
    let _wayland=variable("WAYLAND_DISPLAY")?;
    private_runtime(&socket)?;
    if socket.exists() || fs::symlink_metadata(&socket).is_ok() {
        return Err("session IPC path already occupied".into());
    }
    let nested=env::var("MELANIA_NESTED_SESSION").as_deref()==Ok("1");
    if !nested {check_lock(&bin)?;}
    install_signals();
    let mut services=vec![
        Service::new("control",PathBuf::from("python3"),
            vec![root.join("scripts/melania-labwc-control.py").to_string_lossy().into_owned(),
                "--serve".into(),socket.to_string_lossy().into_owned()],Policy::RecoverCritical),
        Service::new("desktop",bin.join("melania-desktop-surface"),vec![],Policy::RecoverCritical),
        Service::new("panel",bin.join("melania-panel"),vec![],Policy::RecoverCritical),
        Service::new("notify",bin.join("melania-notifyd"),vec![],Policy::RecoverOptional),
    ];
    if !nested{
        services.push(Service::new("idle",bin.join("melania-sessionctl"),
            vec!["idle".into()],Policy::FailClosed));
    }
    if env::var("MELANIA_LABWC_SMOKE_OPEN_FILES").as_deref()==Ok("1"){
        services.push(Service::new("smoke-files",bin.join("melania-files"),
            vec![],Policy::Once));
        println!("MELANIA_LABWC_SMOKE_FILES_PROCESS_STARTED=true");
    }
    let mut error=None;
    // Start the adapter before GTK layers: all panel requests have an
    // accessible, correctly named socket from the first mapped frame.
    if let Err(e)=services[0].start(){error=Some(format!("cannot start control: {e}"));}
    if error.is_none(){
        let deadline=Instant::now()+Duration::from_millis(3500);
        while Instant::now()<deadline && !SHUTDOWN.load(Ordering::Relaxed){
            if fs::symlink_metadata(&socket).is_ok_and(|m|m.file_type().is_socket()){break;}
            if services[0].child.as_mut().is_some_and(|c|
                c.try_wait().is_ok_and(|s|s.is_some())){break;}
            thread::sleep(Duration::from_millis(80));
        }
        if !fs::symlink_metadata(&socket).is_ok_and(|m|m.file_type().is_socket()){
            error=Some("control adapter did not bind IPC".into());
        }
    }
    if error.is_none(){
        for service in services.iter_mut().skip(1){
            if let Err(e)=service.start(){
                if service.policy==Policy::RecoverOptional || service.policy==Policy::Once{
                    eprintln!("MELANIA_LABWC_OPTIONAL_DISABLED={} error={e}",service.name);
                    service.disabled=true;
                } else {error=Some(format!("cannot start {}: {e}",service.name));break;}
            }
        }
    }
    if error.is_none(){
        if !nested{
            // Launch startup apps ONCE per login. Never rerun them when a
            // panel/desktop component recovers from an isolated crash.
            services.push(Service::new("autostart",bin.join("melania-autostart"),
                vec![],Policy::Once));
            if let Some(service)=services.last_mut(){
                if let Err(e)=service.start(){
                    eprintln!("MELANIA_LABWC_AUTOSTART_WARNING={e}");
                    service.disabled=true;
                }
            }
            println!("MELANIA_LABWC_IDLE_GUARD_STARTED=true");
            println!("MELANIA_LABWC_XDG_AUTOSTART_DISPATCHED=true");
        }
        println!("MELANIA_LABWC_SHELL_STARTED=true");
    }
    while error.is_none() && !SHUTDOWN.load(Ordering::Relaxed){
        // A known, private labwc parent must remain alive. Do not switch
        // to another unrelated process if its PID is reused.
        if labwc_owned_pid()!=Ok(labwc){break;}
        let now=Instant::now();
        for index in 0..services.len(){
            // A dead adapter may leave an old socket. Only remove the
            // socket owned by this process before starting the replacement.
            if services[index].name=="control" &&
               services[index].child.as_mut().is_some_and(|c|
                   c.try_wait().is_ok_and(|status|status.is_some())){
                if let Err(e)=clean_socket(&socket){error=Some(e);break;}
            }
            if let Err(e)=services[index].poll(now){error=Some(e);break;}
        }
        if error.is_none(){thread::sleep(POLL);}
    }
    if let Some(ref reason)=error{
        eprintln!("MELANIA_LABWC_SHELL_FAILURE={reason}");
    }
    for service in services.iter_mut().rev(){service.stop();}
    let _=clean_socket(&socket);
    if error.is_some(){terminate_owned_labwc();}
    match error {Some(e)=>Err(e),None=>Ok(())}
}

fn main()->ExitCode {
    match run(){
        Ok(())=>ExitCode::SUCCESS,
        Err(e)=>{eprintln!("MELANIA_LABWC_FATAL={e}"); ExitCode::from(78)}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn restart_is_bounded_and_resets_after_stability(){
        let start=Instant::now();
        let mut budget=RestartBudget::default();
        for delta in [0,10,20]{
            assert!(budget.allow(start+Duration::from_secs(delta)));
        }
        assert!(!budget.allow(start+Duration::from_secs(21)));
        assert!(budget.allow(start+Duration::from_secs(60)));
        assert!(!budget.allow(start+Duration::from_secs(61)));
        assert!(budget.allow(start+Duration::from_secs(121)));
    }
    #[test]fn nested_cannot_start_lock_or_autostart(){
        let source=include_str!("supervisor.rs");
        assert!(source.contains("if !nested {check_lock(&bin)?;}"));
        assert!(source.contains("if !nested{"));
        assert!(source.contains("Policy::FailClosed"));
        assert!(source.contains("Policy::Once"));
        assert!(source.contains("terminate_owned_labwc"));
        assert!(!source.contains("pkill"));
    }
    #[test]fn socket_requires_private_owned_runtime_and_never_follows_symlinks(){
        let dir=std::env::temp_dir().join(format!("melania-supervisor-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let socket=dir.join("x.sock");
        assert!(clean_socket(&socket).is_ok());
        let outside=dir.join("keep");
        fs::write(&outside,"preserve").unwrap();
        std::os::unix::fs::symlink(&outside,&socket).unwrap();
        assert!(clean_socket(&socket).is_err());
        assert_eq!(fs::read_to_string(outside).unwrap(),"preserve");
        fs::remove_dir_all(&dir).unwrap();
    }
}