//! Experimental *seat/hotplug event-loop* test — NOT a native compositor.
//! Must be invoked explicitly in a dedicated TTY or VM; never from a login
//! manager and never as an automatic fallback from the nested compositor.
use std::{env, error::Error, fs, os::unix::fs::FileTypeExt, path::Path, time::Duration};
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
        udev::{UdevBackend, UdevEvent},
    },
    reexports::rustix::fs::OFlags,
    utils::DeviceFd,
};
use melania_native::NativeLifecycle;

const BLOCKED: i32 = 78;
fn blocked(why:&str)->! {
    eprintln!("NATIVE_STAGE1_BLOCKED: {why}");
    std::process::exit(BLOCKED);
}

fn assert_dedicated_console() {
    if env::var("MELANIA_EXPERIMENTAL_SEAT").as_deref()!=Ok("1") {
        blocked("Explicit MELANIA_EXPERIMENTAL_SEAT=1 required");
    }
    if env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some() {
        blocked("Refusing to acquire a libseat session inside an existing desktop");
    }
    // A real Linux VT is required, not SSH, /dev/null or a pseudo-terminal.
    let fd0=fs::read_link("/proc/self/fd/0")
        .unwrap_or_else(|_|blocked("Cannot inspect controlling console"));
    let Some(name)=fd0.file_name().and_then(|x|x.to_str()) else {
        blocked("Controlling console is not a Linux VT");
    };
    let valid=name.strip_prefix("tty").is_some_and(|n|
        !n.is_empty() && n.chars().all(|c|c.is_ascii_digit()));
    if !valid || fd0.parent()!=Some(Path::new("/dev")) {
        blocked("Only a dedicated /dev/ttyN console is permitted");
    }
    let runtime=env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_|blocked("XDG_RUNTIME_DIR missing"));
    let meta=fs::metadata(runtime)
        .unwrap_or_else(|_|blocked("XDG runtime directory not accessible"));
    if !meta.is_dir() {
        blocked("XDG runtime path is not a directory");
    }
    let exists_drm=fs::read_dir("/dev/dri")
        .ok().is_some_and(|entries| entries
            .filter_map(Result::ok)
            .any(|entry|entry.file_name().to_string_lossy().starts_with("card")
                && entry.file_type().is_ok_and(|t|t.is_char_device())));
    if !exists_drm {
        blocked("No DRM card available; use the --capabilities probe first");
    }
}

fn print_capabilities() {
    println!("Melania native stage1 backend (Smithay 0.7)");
    println!("Compiled: libseat session, udev hotplug, DRM/GBM and libinput libraries");
    println!("Present: session pause/resume registry and live GPU discovery");
    println!("Not implemented: KMS output manager, GBM renderer, libinput event routing, VT lifecycle");
    println!("NATIVE_SESSION_READY=false");
}

fn experiment() -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();

    let (session,notifier)=LibSeatSession::new()?;
    let seat_name=session.seat();
    let udev=UdevBackend::new(&seat_name)?;
    let mut state=NativeLifecycle::default();
    if session.is_active() {
        state.on_session(SessionEvent::ActivateSession);
    }
    for (device_id,path) in udev.device_list() {
        state.on_device(melania_native::DeviceChange::Added {
            id: device_id as u64,
            path: path.to_path_buf(),
        });
    }
    println!("Melania native stage1: seat={seat_name}, known GPUs={}",
        state.devices().len());
    // Register the actual Smithay libseat notifier and udev hotplug sources.
    // Nothing opens a DRM FD, creates a framebuffer, or takes over a VT.
    let mut loop_events=calloop::EventLoop::<NativeLifecycle>::try_new()?;
    loop_events.handle().insert_source(notifier,
        |event,_,state| {
            state.on_session(event);
            eprintln!("Melania libseat state: active={}",state.seat_active());
        })?;
    loop_events.handle().insert_source(udev,
        |event,_,state| {
            match &event {
                UdevEvent::Added { path, .. } =>
                    eprintln!("Melania GPU added: {}",path.display()),
                UdevEvent::Removed { .. } =>eprintln!("Melania GPU removed"),
                UdevEvent::Changed { .. } =>eprintln!("Melania GPU changed"),
            }
            state.on_udev(event);
        })?;

    // Finite and opt-in stage1 exercise: no permanent session daemon.
    for _ in 0..30 {
        loop_events.dispatch(Some(Duration::from_millis(100)),&mut state)?;
    }
    println!("Melania native stage1 closed; seen={} GPU events={}",
        state.devices().len(),state.event_count());
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}

/// Stage2: a deliberate, short-lived DRM capability inspection. libseat owns
/// the device open; KMS connectors are NOT reset, mode-set or rendered into.
fn experimental_drm() -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();
    let (mut session,notifier)=LibSeatSession::new()?;
    let udev=UdevBackend::new(session.seat())?;
    let mut scanned=0usize;
    let mut initialized=0usize;
    for (_,path) in udev.device_list() {
        scanned+=1;
        let descriptor=match session.open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        ) {
            Ok(fd)=>fd,
            Err(error)=>{
                eprintln!("Melania DRM: unable to open {} via libseat: {error}",path.display());
                continue;
            },
        };
        let fd=DrmDeviceFd::new(DeviceFd::from(descriptor));
        // false preserves existing connector state: no automatic blanking
        // while inspecting available CRTC scanout controllers.
        match DrmDevice::new(fd,false) {
            Ok((device,_events))=>{
                initialized+=1;
                println!("Melania DRM {}: atomic={} CRTCs={}",
                    path.display(),device.is_atomic(),device.crtcs().len());
                // Device ownership and DRM master return to libseat on drop.
            }
            Err(error)=>{
                eprintln!("Melania DRM: unable to initialize {}: {error}",path.display());
            }
        }
    }
    drop(notifier);
    println!("Melania native stage2: scanned={scanned}, initialized={initialized}");
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}

fn main() {
    match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag] if flag=="--capabilities" =>print_capabilities(),
        [flag] if flag=="--experimental-seat" =>{
            if let Err(error)=experiment(){
                eprintln!("Melania native stage1: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-drm" =>{
            if let Err(error)=experimental_drm(){
                eprintln!("Melania native stage2: {error}");
                std::process::exit(1);
            }
        }
        _ =>{
            eprintln!("Usage: melania-native --capabilities | --experimental-seat | --experimental-drm");
            std::process::exit(2);
        }
    }
}
