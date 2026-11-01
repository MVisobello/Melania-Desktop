//! Experimental *seat/hotplug event-loop* test — NOT a native compositor.
//! Must be invoked explicitly in a dedicated TTY or VM; never from a login
//! manager and never as an automatic fallback from the nested compositor.
use std::{env, error::Error, fs, os::unix::fs::FileTypeExt, path::Path, time::Duration};
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        input::{Device as InputDevice, InputEvent},
        udev::{UdevBackend, UdevEvent},
    },
    reexports::{rustix::fs::OFlags, input::{DeviceCapability, Libinput}},
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

/// Only a QEMU virtual machine may exercise the atomic KMS TEST_ONLY path.
/// DMI is checked before any libseat or DRM device is opened. A caller-set
/// environment variable by itself cannot bypass this hardware gate.
fn assert_qemu_test_guest() {
    if env::var("MELANIA_KMS_TEST_ONLY").as_deref()!=Ok("YES_QEMU_VM") {
        blocked("MELANIA_KMS_TEST_ONLY=YES_QEMU_VM required");
    }
    let vendor=fs::read_to_string("/sys/class/dmi/id/sys_vendor").unwrap_or_default();
    let model=fs::read_to_string("/sys/class/dmi/id/product_name").unwrap_or_default();
    let qemu=["QEMU","Bochs","KVM"].iter().any(|name|
        vendor.contains(name) || model.contains(name));
    if !qemu {
        blocked("Atomic KMS test is restricted to a QEMU/KVM virtual machine");
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
    // Input can be opened ONLY through the same libseat session; no
    // direct host /dev/input access or global keyboard hooks are used.
    let mut input_context=Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        session.clone().into()
    );
    input_context.udev_assign_seat(&seat_name)
        .map_err(|()| "Melania libinput could not assign the selected libseat/udev seat")?;
    let input_backend=LibinputInputBackend::new(input_context.clone());
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
    let mut loop_events=calloop::EventLoop::<NativeLifecycle>::try_new()?;
    // Device enumeration only: input events are not yet routed to Wayland.
    // Record device classes but never store or print keys, text or pointer
    // coordinates. A seat pause invalidates every registered device.
    loop_events.handle().insert_source(input_backend,|event,_,state| {
        match event {
            InputEvent::DeviceAdded { device } => {
                let id=InputDevice::id(&device);
                let caps=melania_native::input_registry::InputCapabilities {
                    keyboard:device.has_capability(DeviceCapability::Keyboard),
                    pointer:device.has_capability(DeviceCapability::Pointer),
                    touch:device.has_capability(DeviceCapability::Touch),
                };
                if caps.any() {
                    let _=state.input_registry_mut().device_added(id,caps);
                }
            }
            InputEvent::DeviceRemoved { device } => {
                state.input_registry_mut().device_removed(&InputDevice::id(&device));
            }
            // Keyboard/mouse/touch payloads are deliberately not forwarded
            // in this experiment; native Wayland seat routing is not ready.
            _ => {}
        }
    })?;
    // Register the actual Smithay libseat notifier and udev hotplug sources.
    // Nothing opens a DRM FD, creates a framebuffer, or takes over a VT.
    loop_events.handle().insert_source(notifier,
        move |event,_,state| {
            match event {
                SessionEvent::PauseSession => input_context.suspend(),
                SessionEvent::ActivateSession => {
                    if let Err(error)=input_context.resume() {
                        eprintln!("Melania libinput resume refused: {error:?}");
                        state.on_session(SessionEvent::PauseSession);
                        return;
                    }
                }
            }
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
    let (keyboards,pointers,touch)=state.input_registry().counts();
    println!("MELANIA_NATIVE_INPUT_ENUMERATED keyboards={keyboards} pointers={pointers} touch={touch} forwarded=false");
    println!("MELANIA_NATIVE_INPUT_WAYLAND_READY=false");
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}

/// Stage2: a deliberate, short-lived DRM capability inspection. libseat owns
/// the device open; KMS connectors are NOT reset, mode-set or rendered into.
fn experimental_drm(render_offscreen: bool) -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();
    let (mut session,notifier)=LibSeatSession::new()?;
    let udev=UdevBackend::new(session.seat())?;
    let mut scanned=0usize;
    let mut initialized=0usize;
    let mut rendered_offscreen=0usize;
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
                // Inspect only: never call set_crtc, reset_state or surface
                // creation. No mode is applied by this probe.
                match melania_native::outputs::inspect_connectors(device.device_fd()) {
                    Ok(connectors) => {
                        println!("Melania DRM connectors on {}: {}",path.display(),connectors.len());
                        for conn in connectors {
                            let suggested=conn.recommendation.as_ref()
                                .map(|m|format!("{}x{}@{}Hz",m.width,m.height,m.refresh_hz))
                                .unwrap_or_else(||"none".into());
                            println!("Melania connector {}: connected={} modes={} candidate={} (NOT applied)",
                                conn.name,conn.connected,conn.advertised_modes.len(),suggested);
                        }
                    }
                    Err(error)=>eprintln!("Melania DRM inventory {}: {error}",path.display()),
                }
                // Determine whether a one-to-one connector/CRTC assignment is
                // even possible before considering any GBM or KMS output.
                // This only reads drm-rs encoder compatibility bitmasks.
                match melania_native::outputs::inspect_output_routes(device.device_fd()) {
                    Ok(routes)=>{
                        let plan=melania_native::outputs::plan_crtc_routes(&routes);
                        println!("Melania DRM viable CRTC routes: {}/{} (NOT activated)",plan.len(),routes.len());
                        for (connector,crtc) in plan {
                            println!("Melania route {connector} => CRTC {crtc} (NOT applied)");
                        }
                    }
                    Err(error)=>eprintln!("Melania DRM routes {}: {error}",path.display()),
                }
                // Stage 7: a CRTC route is useful only if a primary plane
                // advertises the same FourCC + modifier as the offscreen test.
                // This inspection makes no KMS changes or plane claims.
                for plane in melania_native::planes::inspect_primary_planes(&device) {
                    println!("Melania KMS primary {}: planes={} formats={} ABGR8888+LINEAR={} (NO scanout)",
                        plane.crtc,plane.primary_plane_count,plane.supported_format_count,
                        plane.offscreen_abgr_linear_compatible);
                }
                // Stage 5: initialize a real GBM/EGL/GLES context from the
                // same libseat-managed DRM descriptor, then drop it. This is
                // capability detection only: no KMS surface or modesetting.
                match melania_native::renderer::inspect_gbm_egl(device.device_fd().clone()) {
                    Ok(capabilities)=>{
                        println!("Melania GPU renderer: GBM={} EGL/GLES ready, DMA-BUF render formats={} (NO scanout)",
                            capabilities.gbm_backend,capabilities.dmabuf_render_formats);
                    }
                    Err(error)=>eprintln!("Melania GPU renderer unavailable {}: {error}",path.display()),
                }
                if render_offscreen {
                    // Explicitly isolated 64x64 GPU render test: no output
                    // modeset, page flip, KMS surface or monitor interaction.
                    match melania_native::renderer::smoke_render_gbm_offscreen(device.device_fd().clone()) {
                        Ok(())=>{
                            rendered_offscreen+=1;
                            println!("Melania native offscreen GLES frame: OK (NOT displayed)");
                        },
                        Err(error)=>eprintln!("Melania native offscreen GLES frame: {error}"),
                    }
                }
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
    // Offscreen is an explicit rendering test: never return success unless
    // at least one GPU actually rendered and synchronized a frame.
    if render_offscreen && rendered_offscreen==0 {
        return Err("No GPU produced an offscreen frame".into());
    }
    Ok(())
}

/// One short-lived atomic TEST_ONLY and offscreen GLES frame on QEMU/vkms.
/// No frame is submitted to a scanout plane, even if all checks pass.
fn experimental_kms_test() -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();
    assert_qemu_test_guest();
    let (mut session,notifier)=LibSeatSession::new()?;
    let udev=UdevBackend::new(session.seat())?;
    // Seed the same lifecycle state machine used by the stage1 experiment.
    // Capture the initial card identities BEFORE moving udev into calloop.
    let devices=udev.device_list().map(|(id,path)|
        (id as u64,path.to_path_buf())).collect::<Vec<_>>();
    let mut state=NativeLifecycle::default();
    if session.is_active() {
        state.on_session(SessionEvent::ActivateSession);
    }
    for (id,path) in &devices {
        state.on_device(melania_native::DeviceChange::Added {
            id:*id,path:path.clone(),
        });
    }
    let mut event_loop=calloop::EventLoop::<NativeLifecycle>::try_new()?;
    event_loop.handle().insert_source(notifier,|event,_,state| {
        state.on_session(event);
        eprintln!("Melania QEMU libseat event: active={}",state.seat_active());
    })?;
    event_loop.handle().insert_source(udev,|event,_,state| {
        match &event {
            UdevEvent::Added { path, .. } =>
                eprintln!("Melania QEMU DRM hotplug: {}",path.display()),
            UdevEvent::Removed { .. } =>
                eprintln!("Melania QEMU DRM removal: invalidating preflight"),
            UdevEvent::Changed { .. } =>
                eprintln!("Melania QEMU DRM change: invalidating preflight"),
        }
        state.on_udev(event);
    })?;
    let mut completed=false;
    for (id,path) in devices {
        // Process queued session/device events before opening the DRM card.
        event_loop.dispatch(Some(Duration::ZERO),&mut state)?;
        if !session.is_active() || !state.seat_active() {
            return Err("QEMU preflight aborted: inactive libseat session".into());
        }
        let epoch=match melania_native::device_epoch::DeviceEpoch::capture(&state,id,&path) {
            Ok(epoch)=>epoch,
            Err(error)=>{
                eprintln!("Melania QEMU card {} unavailable: {error:?}",path.display());
                continue;
            }
        };
        let descriptor=match session.open(
            &path,OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        ) {
            Ok(fd)=>fd,
            Err(error)=>{eprintln!("Melania KMS VM device {}: {error}",path.display());continue;}
        };
        let fd=DrmDeviceFd::new(DeviceFd::from(descriptor));
        let (mut device,_drm_events)=match DrmDevice::new(fd,false) {
            Ok(result)=>result,
            Err(error)=>{eprintln!("Melania KMS VM init {}: {error}",path.display());continue;}
        };
        let mut invalidated=false;
        let result=melania_native::kms_test::prepare_and_render_test_only(&mut device,|| {
            // Non-blocking drain of pending real libseat and udev events,
            // before/after GPU work. An ioctl may itself still block; the
            // separate QEMU script's watchdog remains mandatory.
            if let Err(error)=event_loop.dispatch(Some(Duration::ZERO),&mut state) {
                invalidated=true;
                return Err(format!("QEMU session event dispatch failed: {error}"));
            }
            if !session.is_active() || epoch.verify(&state).is_err() {
                invalidated=true;
                return Err("QEMU preflight invalidated by seat pause or GPU hotplug".into());
            }
            Ok(true)
        });
        match result {
            Ok(report)=>{
                println!("Melania QEMU atomic TEST_ONLY passed: {} {}x{}, offscreen={} crtc_readable={} baseline_unchanged={} resource_blobs={} borrowed_fbs={} resources_unchanged={} restore_ready={} visible={} rollback_blockers={}",
                    report.connector,report.width,report.height,
                    report.buffer_rendered,report.crtc_state_readable,
                    report.baseline_unchanged,report.copied_blob_count,
                    report.borrowed_framebuffer_count,report.resources_unchanged,
                    report.restoration_implemented,report.visible_frame_submitted,
                    report.rollback_blockers);
                completed=true;
                break;
            }
            Err(error)=>{
                if invalidated {
                    return Err(format!("QEMU preflight aborted without scanout: {error}").into());
                }
                eprintln!("Melania KMS VM preflight {}: {error}",path.display());
            }
        }
    }
    println!("NATIVE_SESSION_READY=false");
    if !completed {
        return Err("No QEMU DRM device passed atomic TEST_ONLY plus offscreen frame".into());
    }
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
            if let Err(error)=experimental_drm(false){
                eprintln!("Melania native stage2: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-offscreen" =>{
            if let Err(error)=experimental_drm(true){
                eprintln!("Melania native stage6 offscreen: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-kms-test" =>{
            if let Err(error)=experimental_kms_test(){
                eprintln!("Melania native stage9 QEMU KMS TEST_ONLY: {error}");
                std::process::exit(1);
            }
        }
        _ =>{
            eprintln!("Usage: melania-native --capabilities | --experimental-seat | --experimental-drm | --experimental-offscreen | --experimental-kms-test");
            std::process::exit(2);
        }
    }
}
