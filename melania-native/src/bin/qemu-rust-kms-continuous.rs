//! Stage 45: explicitly opted-in continuous-generation DRM/KMS receiver.
//!
//! Disposable QEMU-only proof, independent from the existing 1..=6-frame
//! receiver. A successful MLA1 acknowledgement is sent only after the kernel
//! confirms that the submitted framebuffer is on the VM CRTC. No production
//! session, host GPU access, or desktop readiness is implied.
use drm::{
    buffer::{Buffer,DrmFourcc},
    control::{self,Device as ControlDevice,PageFlipFlags,
              connector,crtc,dumbbuffer,framebuffer},
};
use std::{
    error::Error,
    fs::{self,File,OpenOptions},
    os::fd::{AsFd,BorrowedFd},
    os::unix::net::UnixStream,
    io::{Read,Write},
    path::Path,
    thread,
    time::{Duration,Instant},
};

const W:usize=1280;
const H:usize=800;
const FIRST:&str="/run/melania-native-qemu/client-frame.bgra";
const SECOND:&str="/run/melania-native-qemu/client-frame-2.bgra";
const OPT_IN:&str="MELANIA_QEMU_CONTINUOUS_KMS";

struct Card(File);
impl AsFd for Card {
    fn as_fd(&self)->BorrowedFd<'_> {self.0.as_fd()}
}
impl drm::Device for Card {}
impl ControlDevice for Card {}

fn guest_only()->Result<(),Box<dyn Error>> {
    let vendor=fs::read_to_string("/sys/class/dmi/id/sys_vendor").unwrap_or_default();
    let model=fs::read_to_string("/sys/class/dmi/id/product_name").unwrap_or_default();
    let vm=["QEMU","Bochs","KVM"].iter().any(|x|vendor.contains(x)||model.contains(x));
    let vt=fs::read_link("/proc/self/fd/0")
        .map(|p|p==Path::new("/dev/tty1")).unwrap_or(false);
    if !vm || !vt || std::env::var(OPT_IN).as_deref()!=Ok("YES_DISPOSABLE_QEMU") ||
       std::env::var_os("DISPLAY").is_some() ||
       std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Err("MELANIA_RUST_CONTINUOUS_KMS_BLOCKED=QEMU_DMI_TTY1_OPTIN_REQUIRED".into());
    }
    Ok(())
}

fn read_image(path:&str)->Result<Vec<u8>,Box<dyn Error>> {
    let meta=fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len()!=(W*H*4) as u64 {
        return Err(format!("invalid Rust wl_shm snapshot: {path}").into());
    }
    let contents=fs::read(path)?;
    if contents.len()!=W*H*4 {
        return Err("short client-surface file".into());
    }
    Ok(contents)
}

fn upload(card:&Card,pixels:&[u8])
    ->Result<dumbbuffer::DumbBuffer,Box<dyn Error>>
{
    let mut buffer=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let pitch=buffer.pitch() as usize;
    if pitch<W*4 {return Err("DRM dumb buffer pitch too small".into());}
    {
        let mut map=card.map_dumb_buffer(&mut buffer)?;
        let dst=map.as_mut();
        if dst.len()<H*pitch {return Err("DRM mapping shorter than framebuffer".into());}
        for y in 0..H {
            dst[y*pitch..y*pitch+W*4].copy_from_slice(
                &pixels[y*W*4..(y+1)*W*4]
            );
        }
    }
    Ok(buffer)
}

/// On every exit path, attempt to restore the original CRTC before
/// freeing framebuffer objects. A failure is fatal for the VM CI gate.
struct Restore<'a> {
    card:&'a Card,
    original:crtc::Info,
    connector:connector::Handle,
    submitted:bool,
    restored:bool,
}
impl Restore<'_> {
    fn restore(&mut self)->Result<(),Box<dyn Error>> {
        if !self.submitted {
            self.restored=true;
            return Ok(());
        }
        let old=self.original;
        self.card.set_crtc(old.handle(),old.framebuffer(),old.position(),
                           &[self.connector],old.mode())?;
        let observed=self.card.get_crtc(old.handle())?;
        // A matching FB_ID alone does not prove that the original connector
        // is still attached to this CRTC. Verify the encoder -> CRTC route
        // after the restoring modeset, before destroying the guest buffers.
        let connector_attached=self.card.get_connector(self.connector,false)?
            .current_encoder()
            .and_then(|encoder|self.card.get_encoder(encoder).ok())
            .and_then(|encoder|encoder.crtc())==Some(old.handle());
        self.restored=observed.framebuffer()==old.framebuffer()
            && observed.position()==old.position()
            && observed.mode()==old.mode()
            && connector_attached;
        println!("MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED={connector_attached}");
        println!("MELANIA_RUST_KMS_RESTORE_VERIFIED={}",self.restored);
        if !self.restored {return Err("Rust KMS CRTC restore mismatch".into());}
        Ok(())
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.submitted && !self.restored {
            // Best-effort safety recovery even on an intermediate error.
            let _=self.restore();
        }
    }
}

fn wait_for_fb(card:&Card,handle:crtc::Handle,fb:framebuffer::Handle)
    ->Result<(),Box<dyn Error>>
{
    let until=Instant::now()+Duration::from_secs(2);
    while Instant::now()<until {
        if card.get_crtc(handle)?.framebuffer()==Some(fb) {return Ok(());}
        thread::sleep(Duration::from_millis(20));
    }
    Err("page flip did not visibly change CRTC framebuffer".into())
}

fn run()->Result<(),Box<dyn Error>> {
    guest_only()?;
    // Preserve the existing separate-process receiver for historical QEMU
    // CI. The same receiver may now also run in the Smithay server process.
    let stream=UnixStream::connect(
        "/run/melania-native-qemu/continuous-frame-stream.sock"
    )?;
    let card=OpenOptions::new().read(true).write(true).open("/dev/dri/card0")?;
    run_stream(stream,card)
}

/// First real single-process integration checkpoint: the Smithay compositor
/// can give this DRM presenter a pre-connected socket pair without launching
/// an external KMS executable. All QEMU/tty1/opt-in safety checks still apply.
/// This does NOT make direct card0 access a production libseat backend.
pub(crate) fn run_embedded(stream:UnixStream,card:File)->Result<(),Box<dyn Error>> {
    guest_only()?;
    if std::env::var("MELANIA_NATIVE_IN_PROCESS_KMS").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("embedded KMS needs explicit QEMU opt-in".into());
    }
    // QEMU-only Stage109: actual owned wl_shm scene pixels -> GLES texture
    // -> reused GBM DMA-BUF -> DRM page flip -> verified CRTC -> MLA1 ACK.
    // The existing CPU dumb-buffer path remains the default and rollback
    // behavior is unchanged unless this exact VM experiment is requested.
    match std::env::var("MELANIA_NATIVE_GPU_WAYLAND_KMS").ok().as_deref() {
        None=>run_stream(stream,card),
        Some("YES_DISPOSABLE_QEMU")=>{
            if std::env::var("MELANIA_NATIVE_PERSISTENT_KMS").is_ok() ||
               std::env::var("MELANIA_QEMU_CONTINUOUS_FRAMES").as_deref()!=Ok("24") ||
               std::env::var("MELANIA_NATIVE_QEMU_FRAME_TARGET").as_deref()!=Ok("24") {
                return Err("GPU Wayland scene requires explicit finite 24-frame isolated QEMU mode".into());
            }
            melania_native::native_gbm_scanout::qemu_wayland_gpu_scene_stream(
                stream,card.into(),
            )
        }
        Some(_)=>Err("unknown native GPU Wayland opt-in denied".into()),
    }
}

fn run_stream(mut stream:UnixStream,card_file:File)->Result<(),Box<dyn Error>> {
    // The persistent Smithay owner is damage-driven. A compositor that is
    // idle for 12 seconds is still healthy: only a clean transport EOF from
    // signal shutdown may terminate this receiver. The finite QEMU legacy
    // probes continue to fail closed on a stalled frame source.
    let idle_persistent=std::env::var("MELANIA_NATIVE_PERSISTENT_KMS")
        .as_deref()==Ok("YES_DISPOSABLE_QEMU") &&
        std::env::var("MELANIA_NATIVE_IN_PROCESS_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    stream.set_read_timeout(if idle_persistent {
        None
    } else {
        Some(Duration::from_secs(12))
    })?;
    stream.set_write_timeout(Some(Duration::from_secs(7)))?;
    println!("MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true");

    // Embedded mode passes the FD opened via the same live libseat session.
    // The legacy independent receiver still opens its own VM-only descriptor.
    let card=Card(card_file);
    let resources=card.resource_handles()?;
    if resources.crtcs().len()!=1 || resources.connectors().is_empty() ||
       resources.connectors().len()>8 {
        return Err("QEMU stream requires exactly one active CRTC".into());
    }
    let topology=melania_native::native_output_topology::inspect_stable_active_outputs(&card)?;
    let route=topology.single_active()?;
    println!("MELANIA_RUST_KMS_TOPOLOGY_STABLE_OUTPUTS={}",topology.active.len());
    let crtc_handle=route.crtc;
    let original=card.get_crtc(crtc_handle)?;
    let mode=original.mode().ok_or("QEMU CRTC has no active mode")?;
    if mode.size()!=(W as u16,H as u16) || original.framebuffer().is_none() {
        return Err("QEMU streaming requires active 1280x800 framebuffer".into());
    }
    if original.framebuffer()!=Some(route.original_fb) {
        return Err("QEMU streaming original FB_ID changed after topology capture".into());
    }
    let connected=vec![route.connector];

    let mut db1=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let mut db2=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let fb1=card.add_framebuffer(&db1,24,32)?;
    let fb2=card.add_framebuffer(&db2,24,32)?;
    let mut rollback=Restore{
        card:&card,original,connector:connected[0],submitted:false,restored:false
    };
    let mut previous:Option<Vec<u8>>=None;
    let mut flip_ledger=melania_native::native_flip_ledger::NativeFlipLedger::default();
    let mut event_router=melania_native::native_drm_flip::DrmFlipEventRouter::default();

    // Stage84 allows one real GTK image only with the explicit VM opt-in.
    // All historical tests retain the stricter 8..=64-frame count.
    let real_gtk_panel=std::env::var("MELANIA_QEMU_REAL_GTK_PANEL_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    let real_gtk_two_frame=real_gtk_panel && std::env::var("MELANIA_QEMU_GTK_TWO_FRAME_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    let real_gtk_start=real_gtk_two_frame &&
        std::env::var("MELANIA_QEMU_GTK_START_KMS")
            .as_deref()==Ok("YES_QEMU_VM");
    // Do not silently turn invalid or mismatched frame targets into 12.
    // The Wayland sender and DRM receiver must agree before touching KMS.
    let gtk_expected=if real_gtk_start {Some(3)}
        else if real_gtk_two_frame {Some(2)}
        else if real_gtk_panel {Some(1)}
        else {None};
    let configured=std::env::var("MELANIA_QEMU_CONTINUOUS_FRAMES").ok();
    // The new VM-only mode runs until the in-process Wayland owner closes its
    // transport on SIGTERM/SIGINT. No implicit frame budget or host login.
    // Legacy QMP tests keep their exact, finite frame contracts.
    let persistent=std::env::var("MELANIA_NATIVE_PERSISTENT_KMS")
        .as_deref()==Ok("YES_DISPOSABLE_QEMU");
    if persistent && (
        std::env::var("MELANIA_NATIVE_IN_PROCESS_KMS").as_deref()!=Ok("YES_QEMU_VM") ||
        configured.is_some() || gtk_expected.is_some() ||
        std::env::var_os("MELANIA_NATIVE_QEMU_FRAME_TARGET").is_some()
    ) {
        return Err("persistent DRM requires one-process QEMU, no legacy frame budget or GTK proof".into());
    }
    let total=if persistent {None} else {
        Some(melania_native::native_frame_budget::receiver_frame_budget(
            configured.as_deref(),gtk_expected,
        )?)
    };
    let mut frames_seen=0u64;
    let attempt=(||->Result<(),Box<dyn Error>> {
        let mut previous_identity:Option<(u64,u64,u64)>=None;
        // One CRTC+two retained dumb buffers, with bounded transport and
        // kernel event verification on each flip. No ordinal limit here.
        loop {
            if total.is_some_and(|limit|frames_seen>=limit) {break;}
            let index=frames_seen;
            let mut header=[0u8;32];
            if persistent {
                // EOF at a frame boundary is an orderly shutdown request.
                // EOF mid-header/pixels is a fatal truncated transmission.
                if stream.read(&mut header[..1])?==0 {
                    // A desktop may exit before any window has ever been
                    // mapped. No modeset was submitted: restore is a no-op.
                    break;
                }
                stream.read_exact(&mut header[1..])?;
            } else {
                stream.read_exact(&mut header)?;
            }
            if &header[..4]!=b"MLF1" {
                return Err("QEMU continuous KMS frame magic mismatch".into());
            }
            let epoch=u64::from_le_bytes(header[4..12].try_into()?);
            let sequence=u64::from_le_bytes(header[12..20].try_into()?);
            let generation=u64::from_le_bytes(header[20..28].try_into()?);
            let length=u32::from_le_bytes(header[28..32].try_into()?) as usize;
            if length!=W*H*4 || sequence==0 {
                return Err("QEMU continuous KMS payload or ticket invalid".into());
            }
            if previous_identity.is_some_and(|(old_epoch,old_sequence,old_generation)|
                epoch!=old_epoch || sequence<=old_sequence || generation<=old_generation) {
                return Err("QEMU continuous KMS received stale or reordered scene".into());
            }
            let mut pixels=vec![0u8;W*H*4];
            stream.read_exact(&mut pixels)?;
            if previous.as_ref().is_some_and(|last|*last==pixels) {
                return Err("QEMU continuous KMS received identical consecutive pixels".into());
            }
            let framebuffer=if index%2==0 {fb1} else {fb2};
            let target=if index%2==0 {&mut db1} else {&mut db2};
            let pitch=target.pitch() as usize;
            if pitch<W*4 {return Err("invalid continuous dumb buffer pitch".into());}
            {
                let mut map=card.map_dumb_buffer(target)?;
                let bytes=map.as_mut();
                if bytes.len()<H*pitch {
                    return Err("DRM continuous map shorter than image".into());
                }
                for y in 0..H {
                    bytes[y*pitch..y*pitch+W*4].copy_from_slice(
                        &pixels[y*W*4..(y+1)*W*4]
                    );
                }
            }
            if index==0 {
                // Treat any attempt to change scanout as requiring rollback:
                // even a failed ioctl must not bypass the recovery guard.
                rollback.submitted=true;
                card.set_crtc(crtc_handle,Some(framebuffer),
                    original.position(),&connected,Some(mode))?;
            } else {
                // VirtIO DRM may still have the previous asynchronous flip
                // pending even after its FB_ID becomes visible. EAGAIN is
                // transient; retry only this error, with a strict deadline.
                // Other DRM errors fail immediately and no frame is ACKed
                // until wait_for_fb verifies the actual CRTC framebuffer.
                flip_ledger.begin_flip()
                    .map_err(|error|format!("DRM flip already pending or revoked: {error:?}"))?;
                let flip_deadline=Instant::now()+Duration::from_secs(2);
                loop {
                    match card.page_flip(crtc_handle,framebuffer,PageFlipFlags::EVENT,None) {
                        Ok(())=>break,
                        Err(error) if error.raw_os_error()==Some(11) &&
                            Instant::now()<flip_deadline => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(error)=>return Err(error.into()),
                    }
                }
            }
            // The modeset on the first frame is synchronous. Every actual
            // subsequent flip must deliver a DRM page-flip/vblank event
            // before any framebuffer or Wayland callback is acknowledged.
            // This fails closed (no polling-only success) on kernel timeout.
            if index>0 {
                let kernel_frame=melania_native::native_drm_flip::wait_for_kms_page_flip_routed(
                    &card,crtc_handle,Duration::from_secs(2),&mut event_router,
                )?;
                flip_ledger.complete_vm_flip(kernel_frame)
                    .map_err(|error|format!("DRM kernel flip event rejected: {error:?}"))?;
                if flip_ledger.unreliable_vm_counter() {
                    println!("MELANIA_RUST_KMS_VBLANK_COUNTER_UNRELIABLE_QEMU=true");
                }
                println!("MELANIA_RUST_KMS_VBLANK_EVENT_ACK={}",index+1);
            }
            // A vblank alone is insufficient: verify the real scanout state.
            wait_for_fb(&card,crtc_handle,framebuffer)?;
            // Stage96 exercises the SAME kernel-backed verifier exported to
            // the future native output loop: framebuffer ID plus the live
            // connector -> encoder -> CRTC route, not an IPC-only ACK.
            if !melania_native::native_kms_scanout::verify_kernel_scanout(
                &card,crtc_handle,connected[0],framebuffer,
            )? {
                return Err("QEMU continuous KMS scanout lost connector/CRTC route".into());
            }
            if index>0 {
                println!("MELANIA_RUST_KMS_FLIP_LEDGER_CONFIRMED={}",
                         flip_ledger.completed_count());
            }
            println!("MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED={}",index+1);
            println!("MELANIA_RUST_CONTINUOUS_FRAME_VISIBLE={}",index+1);
            if real_gtk_panel && index==0 {
                // The host QMP verifier must capture the LIVE guest CRTC,
                // not a hand-crafted bitmap or a previously saved screenshot.
                if !real_gtk_two_frame {
                    println!("MELANIA_RUST_REAL_GTK_PANEL_FRAME_VISIBLE=1");
                }
            }
            if real_gtk_two_frame {
                println!("MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE={}",index+1);
            }
            if real_gtk_start && index==2 {
                // Unique serial event after the THIRD actual kernel CRTC FB_ID
                // is observed: the host must screendump before CRTC restore.
                println!("MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3");
            }
            // Only the disposable managed-role run emits these markers.
            // The host reads the *actual* QMP GPU framebuffer during the
            // following hold, before any later client frame is presented.
            let managed_role_qmp=std::env::var("MELANIA_QEMU_CONTINUOUS_MANAGED_ROLE_QMP")
                .as_deref()==Ok("YES_QEMU_VM");
            if managed_role_qmp && matches!(index+1,3|5|7|9|13|14) {
                // A physical CRTC/connector scanout has already been
                // verified, and the framebuffer is still owned by QEMU.
                // Frames 13/14 are compositor-driven hide/reveal changes;
                // they carry NO new wl_shm buffer and NO new client callback.
                println!("MELANIA_RUST_CONTINUOUS_MANAGED_ROLE_FRAME_VISIBLE={}",index+1);
            }
            // Real-time serial QMP watcher must receive this while FB is live.
            std::io::stdout().flush()?;
            if std::env::var("MELANIA_QEMU_REAL_LAYER_PANEL")
                    .as_deref()==Ok("YES_QEMU_VM") && index+1==12 {
                println!("MELANIA_RUST_CONTINUOUS_LAYER_FRAME_VISIBLE=12");
                std::io::stdout().flush()?;
            }
            if std::env::var("MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS")
                    .as_deref()==Ok("YES_QEMU_VM") && total==Some(index+1) {
                println!("MELANIA_RUST_CONTINUOUS_TWO_WINDOW_FRAME_VISIBLE={}",index+1);
                std::io::stdout().flush()?;
            }
            if std::env::var("MELANIA_QEMU_CONTINUOUS_DYNAMIC")
                    .as_deref()==Ok("YES_QEMU_VM") && index+1==12 {
                // QMP must capture two different-sized real client
                // framebuffers while the virtual CRTC still scans them out.
                println!("MELANIA_RUST_CONTINUOUS_DYNAMIC_FRAME_VISIBLE={}",index+1);
                std::io::stdout().flush()?;
            }
            let pointer_proof=std::env::var("MELANIA_QEMU_CONTINUOUS_DYNAMIC_POINTER")
                .as_deref()==Ok("YES_QEMU_VM");
            if pointer_proof && index+1==13 {
                println!("MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13");
                std::io::stdout().flush()?;
            }
            let reopen=std::env::var("MELANIA_QEMU_CONTINUOUS_REOPEN")
                .as_deref()==Ok("YES_QEMU_VM");
            if reopen && (index+1==13 || index+1==14) {
                println!("MELANIA_RUST_CONTINUOUS_REOPEN_FRAME_VISIBLE={}",index+1);
                std::io::stdout().flush()?;
            }
            previous=Some(pixels);
            previous_identity=Some((epoch,sequence,generation));
            // Short observable hold for QMP; no 5-second delay per frame.
            // QMP needs time to capture the actual framebuffer on the
            // seventh and twelfth presentations (not just its FB_ID).
            thread::sleep(Duration::from_millis(
                if persistent {0}
                else if (reopen && (index==12 || index==13)) ||
                   (pointer_proof && index==12) ||
                   (managed_role_qmp && matches!(index+1,3|5|7|9|13|14)) ||
                   real_gtk_panel || index==6 || index==11 {
                    if real_gtk_start && index==2 {5500} else {2500}
                   } else {350}
            ));
            if total==Some(index+1) {rollback.restore()?;}
            let mut ack=[0u8;29];
            ack[..4].copy_from_slice(b"MLA1");
            ack[4..12].copy_from_slice(&epoch.to_le_bytes());
            ack[12..20].copy_from_slice(&sequence.to_le_bytes());
            ack[20..28].copy_from_slice(&generation.to_le_bytes());
            ack[28]=0;
            stream.write_all(&ack)?;
            stream.flush()?;
            println!("MELANIA_RUST_CONTINUOUS_FRAME_ACKED={} generation={}",index+1,generation);
            frames_seen=frames_seen.checked_add(1)
                .ok_or("persistent DRM frame counter exhausted")?;
            if pointer_proof && index+1==12 {
                // This exact scenario marker is delivered LIVE to the QMP
                // host via the serial tail of the KMS log; the Smithay
                // server log is only cat-ed later, after the client exits.
                println!("MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_READY_AFTER_KMS12=true");
            }
            std::io::stdout().flush()?;
        }
        Ok(())
    })();

    // Never return success until the original CRTC has been restored.
    let restored=rollback.restore();
    if attempt.is_ok() && restored.is_ok() {
        card.destroy_framebuffer(fb2)?;
        card.destroy_framebuffer(fb1)?;
        card.destroy_dumb_buffer(db2)?;
        card.destroy_dumb_buffer(db1)?;
        if persistent {
            println!("MELANIA_RUST_PERSISTENT_KMS_STOPPED_FRAMES={frames_seen}");
        } else {
            let total=total.expect("finite QEMU proof must have a frame limit");
            println!("MELANIA_RUST_CONTINUOUS_FRAMES_PASS={total}");
        }
        if real_gtk_two_frame {
            println!("MELANIA_RUST_REAL_GTK_TWO_FRAME_KMS_PASS=true");
        }
        if real_gtk_start {
            println!("MELANIA_RUST_REAL_GTK_START_OVERLAY_KMS_PASS=true");
        }
        println!("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false");
        println!("MELANIA_NATIVE_SESSION_READY=false");
        return Ok(());
    }
    attempt?;
    restored?;
    Err("QEMU Rust continuous KMS receiver failed".into())
}

fn main() {
    if let Err(error)=run() {
        eprintln!("MELANIA_RUST_CONTINUOUS_KMS_FAIL={error}");
        std::process::exit(1);
    }
}
