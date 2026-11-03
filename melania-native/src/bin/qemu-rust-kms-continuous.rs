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
        self.restored=observed.framebuffer()==old.framebuffer()
            && observed.position()==old.position()
            && observed.mode()==old.mode();
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
    // Stream is connected while Smithay remains running. Unlike Stage28,
    // the Wayland server must release buffers only after our per-frame ACK.
    let mut stream=UnixStream::connect(
        "/run/melania-native-qemu/continuous-frame-stream.sock"
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(12)))?;
    stream.set_write_timeout(Some(Duration::from_secs(7)))?;
    println!("MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true");

    let card=Card(OpenOptions::new().read(true).write(true).open("/dev/dri/card0")?);
    let resources=card.resource_handles()?;
    if resources.crtcs().len()!=1 || resources.connectors().is_empty() ||
       resources.connectors().len()>8 {
        return Err("QEMU stream requires exactly one active CRTC".into());
    }
    let crtc_handle=resources.crtcs()[0];
    let original=card.get_crtc(crtc_handle)?;
    let mode=original.mode().ok_or("QEMU CRTC has no active mode")?;
    if mode.size()!=(W as u16,H as u16) || original.framebuffer().is_none() {
        return Err("QEMU streaming requires active 1280x800 framebuffer".into());
    }
    let connected=resources.connectors().iter().filter_map(|&conn|{
        let info=card.get_connector(conn,false).ok()?;
        let enc_info=card.get_encoder(info.current_encoder()?).ok()?;
        (enc_info.crtc()==Some(crtc_handle)).then_some(conn)
    }).collect::<Vec<_>>();
    if connected.len()!=1 {
        return Err("QEMU streaming requires one active connector".into());
    }

    let mut db1=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let mut db2=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let fb1=card.add_framebuffer(&db1,24,32)?;
    let fb2=card.add_framebuffer(&db2,24,32)?;
    let mut rollback=Restore{
        card:&card,original,connector:connected[0],submitted:false,restored:false
    };
    let mut previous:Option<Vec<u8>>=None;

    // Only the disposable VM harness sets a bounded target. This is not a
    // compositor frame ordinal; sequence/generation are full-width counters.
    let total: u64 = std::env::var("MELANIA_QEMU_CONTINUOUS_FRAMES")
        .ok().and_then(|value|value.parse().ok())
        .filter(|count:&u64|(8..=64).contains(count))
        .unwrap_or(12);
    let attempt=(||->Result<(),Box<dyn Error>> {
        let mut previous_identity:Option<(u64,u64,u64)>=None;
        for index in 0..total {
            let mut header=[0u8;32];
            stream.read_exact(&mut header)?;
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
                card.set_crtc(crtc_handle,Some(framebuffer),
                    original.position(),&connected,Some(mode))?;
                rollback.submitted=true;
            } else {
                card.page_flip(crtc_handle,framebuffer,PageFlipFlags::empty(),None)?;
            }
            // Only verified CRTC scanout may produce a success ACK.
            wait_for_fb(&card,crtc_handle,framebuffer)?;
            println!("MELANIA_RUST_CONTINUOUS_FRAME_VISIBLE={}",index+1);
            // Real-time serial QMP watcher must receive this while FB is live.
            std::io::stdout().flush()?;
            if std::env::var("MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS")
                    .as_deref()==Ok("YES_QEMU_VM") && index+1==total {
                println!("MELANIA_RUST_CONTINUOUS_TWO_WINDOW_FRAME_VISIBLE={}",index+1);
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
                if (reopen && (index==12 || index==13)) ||
                   index==6 || index==11 {2500} else {350}
            ));
            if index+1==total {rollback.restore()?;}
            let mut ack=[0u8;29];
            ack[..4].copy_from_slice(b"MLA1");
            ack[4..12].copy_from_slice(&epoch.to_le_bytes());
            ack[12..20].copy_from_slice(&sequence.to_le_bytes());
            ack[20..28].copy_from_slice(&generation.to_le_bytes());
            ack[28]=0;
            stream.write_all(&ack)?;
            stream.flush()?;
            println!("MELANIA_RUST_CONTINUOUS_FRAME_ACKED={} generation={}",index+1,generation);
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
        println!("MELANIA_RUST_CONTINUOUS_FRAMES_PASS={total}");
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
