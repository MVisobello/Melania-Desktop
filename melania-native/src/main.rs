//! Experimental *seat/hotplug event-loop* test — NOT a native compositor.
//! Must be invoked explicitly in a dedicated TTY or VM; never from a login
//! manager and never as an automatic fallback from the nested compositor.
use std::{env, error::Error, fs, os::{fd::OwnedFd, unix::fs::{FileTypeExt,MetadataExt}}, path::{Path,PathBuf}, time::Duration};
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        input::{Device as InputDevice, InputEvent},
        udev::{UdevBackend, UdevEvent},
    },
    reexports::{rustix::fs::OFlags, input::{self as libinput, DeviceCapability, Libinput}},
    utils::DeviceFd,
};
use melania_native::NativeLifecycle;
use drm::control::Device as ControlDevice;
mod wayland_bridge;
mod native_client_frame;
// The validated DRM/KMS presenter is compiled into this same executable for
// the explicit one-process QEMU checkpoint, without exposing host login.
#[path="bin/qemu-rust-kms-continuous.rs"]
mod qemu_continuous_receiver;

const BLOCKED: i32 = 78;
fn blocked(why:&str)->! {
    eprintln!("NATIVE_STAGE1_BLOCKED: {why}");
    std::process::exit(BLOCKED);
}

/// Refuse a spoofed or shared XDG runtime directory BEFORE opening libseat,
/// libinput, or a DRM FD. The directory must be a real directory (not a
/// symlink), owned by the running process UID and inaccessible to other users.
/// This is a prerequisite for secure Wayland socket creation, NOT a complete
/// login/session-manager implementation.
fn validate_private_runtime_dir(runtime:&Path)->Result<(),String> {
    let metadata=fs::symlink_metadata(runtime)
        .map_err(|e|format!("XDG runtime directory cannot be inspected: {e}"))?;
    if !metadata.file_type().is_dir() {
        return Err("XDG runtime directory is not a real directory (symlinks refused)".into());
    }
    // /proc/self is kernel-owned and reports the effective credentials of
    // the process; unlike USER/UID environment variables it cannot be spoofed.
    let process=fs::metadata("/proc/self")
        .map_err(|e|format!("Cannot inspect process credentials: {e}"))?;
    if metadata.uid()!=process.uid() {
        return Err("XDG runtime directory is owned by another UID".into());
    }
    if metadata.mode() & 0o077 != 0 {
        return Err("XDG runtime directory is accessible to group/other users".into());
    }
    Ok(())
}

#[cfg(test)]
mod runtime_security_tests {
    use super::*;
    use std::os::unix::fs::{symlink,PermissionsExt};
    #[test]
    fn runtime_rejects_world_accessible_directory_and_symlink() {
        let base=std::env::temp_dir().join(format!(
            "melania-native-runtime-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base,fs::Permissions::from_mode(0o700)).unwrap();
        assert!(validate_private_runtime_dir(&base).is_ok());
        fs::set_permissions(&base,fs::Permissions::from_mode(0o755)).unwrap();
        assert!(validate_private_runtime_dir(&base).is_err());
        fs::set_permissions(&base,fs::Permissions::from_mode(0o700)).unwrap();
        let link=base.with_extension("symlink");
        symlink(&base,&link).unwrap();
        assert!(validate_private_runtime_dir(&link).is_err());
        fs::remove_file(&link).unwrap();
        fs::remove_dir(&base).unwrap();
    }
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
    validate_private_runtime_dir(Path::new(&runtime))
        .unwrap_or_else(|error|blocked(&error));
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

/// Register a device discovered by libinput. Only category and stable sysname
/// are tracked; keycodes, typed text and pointer positions are never stored.
fn register_input_device(
    state:&mut NativeLifecycle,
    device:libinput::Device,
) {
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
    let mut state=NativeLifecycle::default();
    if session.is_active() {
        state.on_session(SessionEvent::ActivateSession);
    }
    // libinput has already enumerated devices during udev_assign_seat.
    // Drain these queued DeviceAdded events before polling its fd; otherwise
    // a quiet keyboard/tablet may never trigger a new readiness callback.
    input_context.dispatch()?;
    for event in &mut input_context {
        if let libinput::Event::Device(libinput::event::DeviceEvent::Added(added))=event {
            let device=libinput::event::EventTrait::device(&added);
            register_input_device(&mut state,device);
        }
    }
    let input_backend=LibinputInputBackend::new(input_context.clone());
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
                register_input_device(state,device);
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

/// Real libseat DRM descriptor acquisition/release/reopen exercise.
/// This probe never calls set_crtc, page_flip or enables Wayland presentation.
/// The QEMU-only forced cycle proves two real seatd opens, NOT a VT switch.
/// A real pause/resume requires an external VT switch and physical validation.
fn experimental_vt_reacquire() -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();
    let forced_cycle=match env::var("MELANIA_NATIVE_READONLY_CYCLE").ok().as_deref() {
        None=>false,
        Some("YES_QEMU_VM")=>{
            assert_qemu_test_guest();
            true
        }
        Some(_)=>blocked("Invalid read-only libseat cycle opt-in"),
    };
    // Optional physical dedicated-VT exercise; the forced QEMU descriptor
    // cycle must always exercise real GBM/EGL/DMA-BUF reconstruction.
    let render_gbm_each_acquire=forced_cycle ||
        match env::var("MELANIA_NATIVE_VT_GBM_ON_REACQUIRE").ok().as_deref() {
            None=>false,
            Some("1")=>true,
            Some(_)=>blocked("MELANIA_NATIVE_VT_GBM_ON_REACQUIRE must be 1"),
        };
    let physical_vt_proof=match env::var("MELANIA_NATIVE_PHYSICAL_VT_PROOF").ok().as_deref() {
        None=>false,
        Some("1") if !forced_cycle=>true,
        Some("1")=>blocked("Physical VT proof is incompatible with the forced QEMU descriptor cycle"),
        Some(_)=>blocked("MELANIA_NATIVE_PHYSICAL_VT_PROOF must be 1"),
    };
    // A physical VT proof MUST rebuild real GBM/DMA-BUFs on the resumed FD.
    let render_gbm_each_acquire=render_gbm_each_acquire || physical_vt_proof;
    let (mut session,notifier)=LibSeatSession::new()?;
    let seat=session.seat().to_string();
    let udev=UdevBackend::new(&seat)?;
    let initial=udev.device_list()
        .map(|(id,path)|(id as u64,path.to_path_buf())).collect::<Vec<_>>();
    let mut state=NativeLifecycle::default();
    if session.is_active() {state.on_session(SessionEvent::ActivateSession);}
    for (id,path) in initial {
        state.on_device(melania_native::DeviceChange::Added{id,path});
    }
    // Physical VT proof must observe a genuine ordered libseat PauseSession
    // followed by ActivateSession; a missing/late udev notification or an
    // inactive epoch is not proof of a kernel VT roundtrip.
    let vt_transitions=std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
    let notifier_transitions=vt_transitions.clone();
    let mut event_loop=calloop::EventLoop::<NativeLifecycle>::try_new()?;
    event_loop.handle().insert_source(notifier,move |event,_,state|{
        use std::sync::atomic::Ordering;
        match event {
            SessionEvent::PauseSession => notifier_transitions.store(1,Ordering::SeqCst),
            SessionEvent::ActivateSession => {
                if notifier_transitions.load(Ordering::SeqCst)==1 {
                    notifier_transitions.store(2,Ordering::SeqCst);
                }
            }
        }
        state.on_session(event);
    })?;
    event_loop.handle().insert_source(udev,|event,_,state|{
        state.on_udev(event);
    })?;

    // The owned FD stays inside this probe; never persist across libseat
    // pause, GPU hotplug, or another acquisition epoch. No DMA-BUF is made.
    let mut owned:Option<(melania_native::device_epoch::DeviceEpoch,OwnedFd)>=None;
    let mut opened=0u32;
    // Bind every acquisition epoch to the same real kernel character-node
    // identity, not merely the reusable /dev/dri/cardN pathname.
    let mut first_drm_node:Option<melania_native::native_drm_node_identity::DrmNodeIdentity>=None;
    // The physical VT proof must restore *every* initially active output,
    // not merely render whatever subset is still active after the switch.
    let mut baseline_output_topology:Option<melania_native::native_output_topology::NativeOutputTopology>=None;
    // Keep the original selected path while the seat-owned descriptor is
    // alive. Re-check the kernel identity at every dispatch, not only during
    // GBM/EGL allocation: udev notification may lag a device replacement.
    let mut live_card_path:Option<PathBuf>=None;
    let mut observed_pause=false;
    let mut cycled=false;
    let mut physical_resume_verified=false;
    let cycles=if forced_cycle {40} else if physical_vt_proof {600} else {300};
    for _ in 0..cycles {
        // Even while polling for real libseat pause/hotplug, release the
        // descriptor explicitly if the event loop itself fails.
        if let Err(error)=event_loop.dispatch(Some(Duration::from_millis(100)),&mut state) {
            if let Some((_,fd))=owned.take() {session.close(fd)?;}
            return Err(error.into());
        }
        let active=session.is_active() && state.seat_active();
        if active {
            if let Some((_,fd))=owned.as_ref() {
                let path=live_card_path.as_deref()
                    .ok_or("Seat-owned DRM descriptor has no recorded device path")?;
                let baseline=first_drm_node
                    .ok_or("Seat-owned DRM descriptor has no verified device identity")?;
                if let Err(error)=melania_native::native_drm_node_identity::verify_libseat_drm_primary_card(
                    path,fd,Some(baseline)
                ) {
                    // A live device substitution is not a normal VT pause.
                    // Close the real libseat FD and refuse any new GPU work.
                    let (_,fd)=owned.take().expect("live libseat DRM descriptor");
                    session.close(fd)?;
                    return Err(format!("Live DRM device identity invalidated: {error}").into());
                }
            }
        }
        if let Some((epoch,_))=owned.as_ref() {
            if !active || epoch.verify(&state).is_err() {
                let unexpected_change=active;
                let (_,fd)=owned.take().expect("owned read-only descriptor");
                live_card_path=None;
                // On any libseat pause, release our actual DRM descriptor.
                // If seatd refuses the release, abort without new opens.
                session.close(fd)?;
                if unexpected_change {
                    return Err("DRM GPU changed while seat remained active; refusing stale reopen".into());
                }
                observed_pause=true;
                println!("MELANIA_NATIVE_VT_READONLY_PAUSED=true");
            }
        }
        if active && owned.is_none() {
            let watch=melania_native::native_gpu_watch::NativeGpuWatch::from_discovered(
                state.devices().iter().map(|(id,path)|(*id,path.clone())),
            );
            let card_path=watch.single_primary_card()
                .map_err(|err|format!("Read-only DRM GPU selection refused: {err:?}"))?
                .to_path_buf();
            let id=*state.devices().iter()
                .find(|(_,path)|**path==card_path)
                .map(|(id,_)|id)
                .ok_or("Read-only DRM card no longer present on seat")?;
            let epoch=melania_native::device_epoch::DeviceEpoch::capture(
                &state,id,&card_path,
            ).map_err(|err|format!("Read-only DRM epoch unavailable: {err:?}"))?;
            // Actual libseat/seatd acquisition, NEVER a direct /dev/dri open.
            let fd=session.open(&card_path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)?;
            let kernel_node=match melania_native::native_drm_node_identity::verify_libseat_drm_primary_card(
                &card_path,&fd,first_drm_node
            ) {
                Ok(node)=>node,
                Err(error)=>{
                    session.close(fd)?;
                    return Err(error.into());
                }
            };
            // Keep a single libseat-issued DRM FD alive while rebuilding
            // the renderer on a fresh cloned descriptor. GBM/EGL, its real
            // DMA-BUF allocations and GPU fences are dropped completely
            // BEFORE libseat closes the original FD. On ANY error, close
            // the owned seat descriptor rather than escaping via '?'.
            let acquired=(|| -> Result<(usize,Vec<melania_native::native_output_probe_plan::OutputBufferProbe>,Option<melania_native::native_output_topology::NativeOutputTopology>),Box<dyn Error>> {
                let card=DrmDeviceFd::new(DeviceFd::from(fd.try_clone()?));
                let mut verified_output_topology=None;
                let crtcs=card.resource_handles()?.crtcs().len();
                if crtcs==0 {
                    return Err("Libseat-owned DRM card has no CRTC resources".into());
                }
                let probes=if render_gbm_each_acquire {
                    // Read the actual kernel connector->encoder->CRTC->FB
                    // mapping, rather than rendering a guessed 640x480 surface.
                    // Every independently active CRTC must be renderable;
                    // one failed output rejects the entire reacquisition.
                    let before=melania_native::native_output_topology::inspect_stable_active_outputs(&card)?;
                    let plan=melania_native::native_output_probe_plan::plan_active_output_probes(&before)?;
                    // A physical VT resume cannot be accepted if no active
                    // CRTC has produced a real output-sized GBM/DMA-BUF.
                    // Headless discovery is valid for diagnostics, not proof
                    // that a visible desktop recovered after VT activation.
                    if physical_vt_proof && plan.is_empty() {
                        return Err("Physical VT resume has no active DRM output to validate".into());
                    }
                    for probe in &plan {
                        melania_native::renderer::smoke_render_gbm_output_buffers(
                            card.clone(),probe.width,probe.height,2
                        ).map_err(|error|format!(
                            "Fresh libseat DRM FD failed GBM/EGL/DMA-BUF output {:?} ({}x{}): {error}",
                            probe.crtc,probe.width,probe.height
                        ))?;
                    }
                    // Do not count allocations if an external modeset,
                    // hotplug or replaced original FB happened while the
                    // EGL/GBM resources were being rebuilt.
                    let after=melania_native::native_output_topology::inspect_stable_active_outputs(&card)?;
                    if before!=after {
                        return Err("Active DRM CRTC/connector/framebuffer changed during GPU reacquisition".into());
                    }
                    if physical_vt_proof {
                        verified_output_topology=Some(after);
                    }
                    plan
                } else { Vec::new() };
                Ok((crtcs,probes,verified_output_topology))
            })();
            let (crtcs,probes,reacquired_topology)=match acquired {
                Ok(value)=>value,
                Err(error)=>{
                    session.close(fd)?;
                    return Err(error);
                }
            };
            // Verify that an actual post-VT restore has preserved the complete
            // original connector/encoder/CRTC set and dimensions. A new FB_ID
            // is allowed: a different VT may have used its own framebuffer.
            // Rejection closes the libseat-owned descriptor before returning.
            if let Some(topology)=reacquired_topology {
                if let Some(baseline)=baseline_output_topology.as_ref() {
                    if let Err(error)=melania_native::native_output_probe_plan::verify_vt_output_restore(
                        baseline,&topology
                    ) {
                        session.close(fd)?;
                        return Err(format!("Physical VT multi-output restore refused: {error}").into());
                    }
                    println!("MELANIA_NATIVE_VT_ALL_ORIGINAL_OUTPUTS_RESTORED=true count={}",topology.active.len());
                } else {
                    baseline_output_topology=Some(topology);
                }
            }
            // A notifier/udev polling error must also release the actual
            // seatd-owned FD before aborting: never strand GPU ownership.
            if let Err(error)=event_loop.dispatch(Some(Duration::ZERO),&mut state) {
                session.close(fd)?;
                return Err(error.into());
            }
            if !session.is_active() || epoch.verify(&state).is_err() {
                session.close(fd)?;
                return Err("Seat/GPU invalidated during real DRM reacquisition".into());
            }
            // Detect a replaced device node EVEN when udev delivered no
            // notification during GPU initialization. Refuse stale handles
            // rather than treating a matching /dev/dri path as ownership.
            if let Err(error)=melania_native::native_drm_node_identity::verify_libseat_drm_primary_card(
                &card_path,&fd,Some(kernel_node)
            ) {
                session.close(fd)?;
                return Err(error.into());
            }
            let next_open=match opened.checked_add(1) {
                Some(next)=>next,
                None=>{
                    session.close(fd)?;
                    return Err("DRM acquisition counter overflow".into());
                }
            };
            opened=next_open;
            first_drm_node=Some(kernel_node);
            live_card_path=Some(card_path.clone());
            println!("MELANIA_NATIVE_VT_DRM_FD_NODE_VERIFIED={opened} rdev={}",kernel_node.rdev());
            // A physical proof can only complete after an actual libseat
            // PauseSession was observed and a NEW seat-owned FD has passed
            // every real output-sized GPU render/sync and epoch check.
            if physical_vt_proof && observed_pause && opened>=2 &&
                vt_transitions.load(std::sync::atomic::Ordering::SeqCst)==2 {
                physical_resume_verified=true;
                println!("MELANIA_NATIVE_PHYSICAL_VT_RESUME_GBM_VERIFIED=true");
            }
            println!("MELANIA_NATIVE_VT_READONLY_DRM_OPEN={} crtcs={crtcs}",opened);
            if render_gbm_each_acquire {
                println!("MELANIA_NATIVE_VT_GBM_EGL_DMA_BUF_REBUILT={opened} frames=2");
                println!("MELANIA_NATIVE_VT_REAL_OUTPUTS_REBUILT={opened} count={}",probes.len());
                for probe in &probes {
                    println!("MELANIA_NATIVE_VT_REAL_OUTPUT_GBM crtc={:?} {}x{} frames=2",
                             probe.crtc,probe.width,probe.height);
                }
            }
            owned=Some((epoch,fd));
        }
        if forced_cycle && !cycled && owned.is_some() {
            // This tests a real FD close+new libseat open on VirtIO without
            // faking a PauseSession event or claiming a VT acceptance test.
            let (_,fd)=owned.take().expect("first DRM acquisition");
            live_card_path=None;
            session.close(fd)?;
            cycled=true;
            println!("MELANIA_NATIVE_VT_READONLY_QEMU_CYCLE=true");
        }
        if forced_cycle && cycled && opened==2 {break;}
        if physical_resume_verified {break;}
    }
    if let Some((_,fd))=owned.take() {session.close(fd)?;}
    if physical_vt_proof && !physical_resume_verified {
        return Err("No genuine libseat VT pause and GPU-verified DRM resume within the physical test window".into());
    }
    if forced_cycle && (!cycled || opened!=2) {
        return Err("QEMU real libseat DRM descriptor reopen not observed".into());
    }
    println!("MELANIA_NATIVE_VT_READONLY_ACQUISITIONS={opened}");
    println!("MELANIA_NATIVE_VT_READONLY_ACTUAL_PAUSE_OBSERVED={observed_pause}");
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}

/// Stage2: a deliberate, short-lived DRM capability inspection. libseat owns
/// the device open; KMS connectors are NOT reset, mode-set or rendered into.
fn experimental_drm(render_offscreen: bool, output_sized: bool) -> Result<(),Box<dyn Error>> {
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
        // Keep the original libseat-issued device descriptor registered
        // until every DRM/GBM/EGL clone has been dropped. A bare close(2)
        // on DrmDeviceFd does NOT notify seatd/libseat of device release.
        // This scope also covers the early invalid-output-plan path.
        let probe=(|| -> Result<(),Box<dyn Error>> {
        let fd=DrmDeviceFd::new(DeviceFd::from(descriptor.try_clone()?));
        // false preserves existing connector state: no automatic blanking
        // while inspecting available CRTC scanout controllers.
        match DrmDevice::new(fd,false) {
            Ok((device,_events))=>{
                initialized+=1;
                println!("Melania DRM {}: atomic={} CRTCs={}",
                    path.display(),device.is_atomic(),device.crtcs().len());
                // A usable physical output requires a stable, kernel-read
                // connector -> encoder -> CRTC -> original FB_ID route, not
                // merely an EDID mode. The double read detects topology
                // changes during the snapshot. It never modesets or claims
                // a display. Mirrors/shared CRTCs fail closed until explicit
                // per-output KMS ownership is implemented.
                let active_output_plan=match
                    melania_native::native_output_topology::inspect_stable_active_outputs(
                        device.device_fd()
                    ) {
                    Ok(topology)=>{
                        println!("MELANIA_NATIVE_ACTIVE_DRM_ROUTES={} (READ_ONLY)",
                                 topology.active.len());
                        for route in &topology.active {
                            println!("MELANIA_NATIVE_ACTIVE_DRM_ROUTE connector={:?} crtc={:?} original_fb={:?} size={}x{} (NOT MODIFIED)",
                                     route.connector,route.crtc,route.original_fb,
                                     route.width,route.height);
                        }
                        match melania_native::native_output_probe_plan::plan_active_output_probes(&topology) {
                            Ok(probes)=>Some(probes),
                            Err(error)=>{
                                eprintln!("MELANIA_NATIVE_ACTIVE_DRM_PROBE_PLAN_REFUSED: {error}");
                                None
                            },
                        }
                    },
                    Err(error)=>{
                        eprintln!("MELANIA_NATIVE_ACTIVE_DRM_ROUTE_UNVERIFIED: {error}");
                        None
                    },
                };
                // Output-sized allocation must cover EVERY active CRTC
                // independently, using the actual kernel scanout dimensions,
                // or none of them count as a successful multi-output probe.
                if output_sized && active_output_plan.is_none() {
                    eprintln!("Melania native output-sized probe refused unstable or unrenderable physical DRM topology");
                    return Ok(());
                }
                // Inspect only: never call set_crtc, reset_state or surface
                // creation. No mode is applied by this probe.
                let mut probe_mode=None;
                // Prefer the mode that the kernel is ALREADY scanning out.
                // VirtIO may advertise 5120x2160 EDID modes while the live
                // CRTC runs at 1280x800. Never let an oversized advertised
                // mode bypass the GBM allocator budget or trigger modeset.
                let active_size=if output_sized {
                    device.device_fd().resource_handles().ok().and_then(|resources|
                        resources.crtcs().iter().find_map(|&crtc|
                            device.device_fd().get_crtc(crtc).ok()
                                .and_then(|info| info.mode().map(|mode| mode.size()))
                        )
                    )
                } else {None};
                match melania_native::outputs::inspect_connectors(device.device_fd()) {
                    Ok(connectors) => {
                        println!("Melania DRM connectors on {}: {}",path.display(),connectors.len());
                        for conn in connectors {
                            if output_sized && conn.connected {
                                // Align the selection and the renderer's
                                // dimension limits; budget alone would accept
                                // an EDID width over the 4096 GPU safety cap.
                                let supported=conn.advertised_modes.iter().filter(|mode|
                                    melania_native::renderer::validate_gbm_output_dimensions(
                                        mode.width,mode.height,2
                                    ).is_ok()
                                ).cloned().collect::<Vec<_>>();
                                let active_candidates=supported.iter().filter(|mode|
                                    active_size.is_some_and(|(width,height)|
                                        mode.width==u32::from(width) &&
                                        mode.height==u32::from(height))
                                ).cloned().collect::<Vec<_>>();
                                let candidates=if active_candidates.is_empty() {
                                    &supported
                                } else {&active_candidates};
                                let candidate=melania_native::outputs::recommended_mode_for_budget(
                                    candidates,2,96*1024*1024
                                );
                                if let Some(mode)=candidate {
                                    let score=|m:&melania_native::outputs::DisplayMode|(
                                        u64::from(m.width)*u64::from(m.height),m.refresh_hz
                                    );
                                    if probe_mode.as_ref().is_none_or(|old|score(&mode)>score(old)) {
                                        probe_mode=Some(mode);
                                    }
                                }
                            }
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
                    // Only real EGL/GBM DMA-BUF rendering. Both probes are
                    // bounded and cannot submit a KMS page flip or modeset.
                    // In output-sized mode, use a connected EDID candidate;
                    // no hard-coded fallback resolution may mask a GPU error.
                    let rendered=if output_sized {
                        // The EDID recommendation above is diagnostic only.
                        // Render and synchronize two real GBM/GLES frames for
                        // every connector's active kernel mode. A partial
                        // success cannot claim full physical GPU coverage.
                        let plan=active_output_plan.as_ref()
                            .expect("output-sized DRM route plan validated above");
                        let all:Result<(),String>=plan.iter().try_for_each(|probe|{
                            melania_native::renderer::smoke_render_gbm_output_buffers(
                                device.device_fd().clone(),probe.width,probe.height,2
                            )?;
                            println!("MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED={}x{}x2",
                                     probe.width,probe.height);
                            println!("MELANIA_NATIVE_GBM_ACTIVE_CRTC_PROBED={:?}",
                                     probe.crtc);
                            Ok(())
                        });
                        if all.is_ok() {
                            println!("MELANIA_NATIVE_GBM_ALL_ACTIVE_OUTPUTS_SYNCHRONIZED={}",
                                     plan.len());
                        }
                        all
                    } else {
                        melania_native::renderer::smoke_render_gbm_offscreen(
                            device.device_fd().clone()
                        )
                    };
                    match rendered {
                        Ok(())=>{
                            rendered_offscreen+=1;
                            println!("Melania native offscreen GLES frames: OK (NOT displayed)");
                        },
                        Err(error)=>eprintln!("Melania native offscreen GLES frames: {error}"),
                    }
                }
                // DRM/EGL/GBM clones are dropped on exit from this scope.
            }
            Err(error)=>{
                eprintln!("Melania DRM: unable to initialize {}: {error}",path.display());
            }
        }
        Ok(())
        })();
        // Every successful libseat.open MUST have an explicit libseat.close,
        // including failed initialization, invalid output plans, GPU render
        // failures and clone errors. Report close failure, never continue to
        // another card with a potentially unretired seatd device token.
        let released=session.close(descriptor);
        released?;
        println!("MELANIA_NATIVE_STAGE2_LIBSEAT_DRM_CLOSED=true");
        probe?;
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

/// Two real GPU-rendered framebuffer scanouts through the QEMU CRTC.
/// The production login path remains blocked; original device ownership
/// stays with libseat until the kernel mode and connector are restored.
fn experimental_gbm_kms_scanout() -> Result<(),Box<dyn Error>> {
    assert_dedicated_console();
    assert_qemu_test_guest();
    if env::var("MELANIA_NATIVE_VM_GBM_KMS").as_deref()
        !=Ok("YES_DISPOSABLE_QEMU") {
        blocked("explicit MELANIA_NATIVE_VM_GBM_KMS=YES_DISPOSABLE_QEMU required");
    }
    let (mut session,notifier)=LibSeatSession::new()?;
    if !session.is_active() {
        return Err("QEMU GPU scanout requires an active libseat session".into());
    }
    let card_path=Path::new("/dev/dri/card0");
    let fd=session.open(
        card_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    // The scanout probe must not trust the pathname alone: require the
    // actual libseat-issued FD to be this kernel DRM primary card before
    // any GBM allocation or KMS modeset can be attempted.
    let original_node=match melania_native::native_drm_node_identity::verify_libseat_drm_primary_card(
        card_path,&fd,None
    ) {
        Ok(node)=>node,
        Err(error)=>{
            session.close(fd)?;
            return Err(error.into());
        }
    };
    // The native GPU/KMS probe must finish (and restore the CRTC)
    // before the original libseat device is released.
    let cloned=fd.try_clone()?;
    // This separate opt-in exercises twenty-four real GPU/KMS presentations
    // without enabling native host login or altering the two-frame smoke.
    let stress=env::var("MELANIA_NATIVE_VM_GBM_STRESS").ok();
    let outcome=match stress.as_deref() {
        None=>melania_native::native_gbm_scanout::qemu_two_gpu_frames(cloned),
        Some("YES_DISPOSABLE_QEMU")=>
            melania_native::native_gbm_scanout::qemu_stress_gpu_frames(cloned,24),
        Some(_)=>Err("invalid QEMU GBM stress opt-in".into()),
    };
    // Even a successful real KMS presentation must not be accepted if the
    // node was removed/replaced while the scanout/rollback routine ran.
    // Do not let this check skip the existing rollback: the KMS routine
    // has already returned before this second identity inspection.
    let still_same=melania_native::native_drm_node_identity::verify_libseat_drm_primary_card(
        card_path,&fd,Some(original_node)
    );
    let released=session.close(fd);
    drop(notifier);
    println!("MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED={}",released.is_ok());
    released?;
    outcome?;
    still_same?;
    println!("MELANIA_NATIVE_GBM_KMS_DRM_FD_IDENTITY_VERIFIED=true");
    println!("NATIVE_SESSION_READY=false");
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
        // The seatd-owned original stays registered while a cloned DRM FD
        // performs TEST_ONLY. Always close the original through libseat after
        // destroying all temporary kernel/GPU handles, including failures.
        let tested=(|| -> Result<bool,Box<dyn Error>> {
        let fd=DrmDeviceFd::new(DeviceFd::from(descriptor.try_clone()?));
        let (mut device,_drm_events)=match DrmDevice::new(fd,false) {
            Ok(result)=>result,
            Err(error)=>{
                eprintln!("Melania KMS VM init {}: {error}",path.display());
                return Ok(false);
            }
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
                Ok(true)
            }
            Err(error)=>{
                if invalidated {
                    return Err(format!("QEMU preflight aborted without scanout: {error}").into());
                }
                eprintln!("Melania KMS VM preflight {}: {error}",path.display());
                Ok(false)
            }
        }
        })();
        // Releasing through libseat is mandatory even if the test returned
        // an error. The cloned DrmDevice/GBM resources have gone out of scope.
        session.close(descriptor)?;
        println!("MELANIA_NATIVE_KMS_TEST_LIBSEAT_DRM_CLOSED=true");
        if tested? {completed=true;break;}
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
        [flag] if flag=="--experimental-vt-reacquire" =>{
            if let Err(error)=experimental_vt_reacquire() {
                eprintln!("Melania read-only libseat DRM reacquisition: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-drm" =>{
            if let Err(error)=experimental_drm(false,false){
                eprintln!("Melania native stage2: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-offscreen" =>{
            if let Err(error)=experimental_drm(true,false){
                eprintln!("Melania native stage6 offscreen: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-gbm-kms-scanout" =>{
            if let Err(error)=experimental_gbm_kms_scanout() {
                eprintln!("Melania VM GPU scanout: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-gbm-output" =>{
            if let Err(error)=experimental_drm(true,true){
                eprintln!("Melania native output-sized GPU test: {error}");
                std::process::exit(1);
            }
        }
        [flag] if flag=="--experimental-wayland-seat" =>{
            assert_dedicated_console();
            assert_qemu_test_guest();
            if env::var("MELANIA_NATIVE_WAYLAND_EXPERIMENT").as_deref()!=Ok("YES_QEMU_VM") {
                blocked("MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM required");
            }
            if let Err(error)=wayland_bridge::run(){
                eprintln!("Melania QEMU Wayland input bridge: {error}");
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
            eprintln!("Usage: melania-native --capabilities | --experimental-seat | --experimental-vt-reacquire | --experimental-drm | --experimental-offscreen | --experimental-gbm-output | --experimental-gbm-kms-scanout | --experimental-kms-test");
            std::process::exit(2);
        }
    }
}
