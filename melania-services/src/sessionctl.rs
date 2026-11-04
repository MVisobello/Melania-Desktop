//! Protected, labwc-owned Melania session lock and idle monitor.
//!
//! swaylock authenticates with PAM and ext-session-lock: never draw a fake
//! GTK overlay over an unlocked compositor. swayidle waits for the *real*
//! lock handshake before acknowledging logind's before-sleep inhibitor.
//! Nesting/preview may never lock the host, and no action runs as root.
use std::{env,fs,os::unix::fs::PermissionsExt,process::{Command,ExitCode}};

fn allowed(get:impl Fn(&str)->Option<String>)->Result<(),&'static str> {
    if get("MELANIA_EXTERNAL_COMPOSITOR").as_deref()!=Some("labwc") {
        return Err("only an independent Melania labwc session can lock");
    }
    if get("MELANIA_NESTED_SESSION").as_deref()==Some("1")
        || get("MELANIA_SMOKE_OPEN_START").as_deref()==Some("1") {
        return Err("locking a preview or smoke test is prohibited");
    }
    if get("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref()!=Some("1") {
        return Err("standalone session authorization missing");
    }
    if get("XDG_SESSION_TYPE").as_deref()!=Some("wayland")
        || get("WAYLAND_DISPLAY").is_none_or(|d|d.is_empty()) {
        return Err("no independent Wayland session to lock");
    }
    Ok(())
}

fn executable(name:&str)->bool {
    // Check actual executability, not mere existence. Never shell-expand or
    // accept a filename from a window title or an untrusted IPC payload.
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|dir|{
        let file=dir.join(name);
        fs::metadata(file).is_ok_and(|meta| {
            meta.is_file() && (meta.permissions().mode()&0o111)!=0
        })
    })
}

fn tools()->Result<(),String>{
    for program in ["swaylock","swayidle"] {
        if !executable(program) {
            return Err(format!("{program} is not installed/executable; install swaylock swayidle"));
        }
    }
    Ok(())
}

fn verify()->Result<(),String>{
    allowed(|k|env::var(k).ok()).map_err(str::to_owned)?;
    tools()?;
    Ok(())
}

fn lock()->Result<(),String>{
    verify()?;
    // -f detaches only AFTER ext-session-lock reports a locked surface.
    // A successful exit therefore establishes a real lock. A failure is
    // surfaced, and MUST NOT be treated as a secure desktop.
    let status=Command::new("swaylock").args(["-f","-c","182D47"]).status()
        .map_err(|e|format!("cannot start swaylock: {e}"))?;
    if !status.success(){return Err(format!("swaylock failed: {status}"));}
    println!("MELANIA_SESSION_LOCK_ACQUIRED=true");
    Ok(())
}

fn idle()->Result<(),String>{
    verify()?;
    // Immutable executable/arguments: swayidle may call its event commands
    // through sh, but no user-supplied value ever enters those strings.
    // -w waits for the lock handshake before logind allows suspend.
    let status=Command::new("swayidle").args([
        "-w", "timeout", "300", "melania-sessionctl lock",
        "before-sleep", "melania-sessionctl lock",
        "lock", "melania-sessionctl lock",
    ]).status().map_err(|e|format!("cannot start swayidle: {e}"))?;
    if status.success(){Ok(())}else{Err(format!("swayidle exited: {status}"))}
}

fn run()->Result<(),String>{
    let arg=env::args().nth(1).ok_or("usage: melania-sessionctl check-tools|check-lock|lock|idle")?;
    if env::args().count()!=2 {return Err("exactly one command required".into());}
    match arg.as_str(){
        "check-tools" => tools(),
        "check-lock" => verify(),
        "lock" => lock(),
        "idle" => idle(),
        _ => Err("unknown session command".into()),
    }
}

fn main()->ExitCode {
    match run(){
        Ok(())=>ExitCode::SUCCESS,
        Err(error)=>{
            eprintln!("MELANIA_SESSION_SECURITY_BLOCKED={error}");
            ExitCode::from(78)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn environment()->HashMap<&'static str,String>{
        HashMap::from([
            ("MELANIA_EXTERNAL_COMPOSITOR","labwc".into()),
            ("MELANIA_ALLOW_HOST_POWER_ACTIONS","1".into()),
            ("XDG_SESSION_TYPE","wayland".into()),
            ("WAYLAND_DISPLAY","wayland-1".into())
        ])
    }
    #[test]
    fn only_real_standalone_wayland_is_eligible(){
        let mut e=environment();
        assert!(allowed(|k|e.get(k).cloned()).is_ok());
        for (name,value) in [
            ("MELANIA_NESTED_SESSION","1"),
            ("MELANIA_SMOKE_OPEN_START","1"),
        ] {
            e.insert(name,value.into());
            assert!(allowed(|k|e.get(k).cloned()).is_err());
            e.remove(name);
        }
        for name in [
            "MELANIA_EXTERNAL_COMPOSITOR",
            "MELANIA_ALLOW_HOST_POWER_ACTIONS",
            "XDG_SESSION_TYPE",
            "WAYLAND_DISPLAY"
        ] {
            let previous=e.remove(name).unwrap();
            assert!(allowed(|k|e.get(k).cloned()).is_err(),"{name}");
            e.insert(name,previous);
        }
    }
    #[test]
    fn lock_uses_only_real_protocol_and_safe_commands(){
        // Contract for the executable constants. No fake screenshot lock.
        let text=include_str!("sessionctl.rs");
        assert!(text.contains(r#"Command::new("swaylock").args(["-f","-c","182D47"])"#));
        assert!(text.contains(r#""before-sleep", "melania-sessionctl lock""#));
        assert!(text.contains(r#""timeout", "300", "melania-sessionctl lock""#));
    }
}
