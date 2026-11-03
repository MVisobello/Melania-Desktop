//! Stage 28: two successive *client-owned* Wayland frames displayed on the
//! disposable QEMU VirtIO GPU by Rust DRM/KMS, without the C scanout helper.
 //!
 //! This is a deliberately bounded VM experiment, not Melania's production
 //! renderer. The Wayland server has previously copied TWO distinct wl_shm
 //! commits into private guest files; this binary only submits those pixels
 //! to KMS. No display-manager session, hotplug, libseat VT recovery or
 //! GBM/EGL animation is claimed. Preserve the original console CRTC.
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
const OPT_IN:&str="MELANIA_QEMU_RUST_LIVE_STREAM_KMS";

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
        return Err("MELANIA_RUST_STREAM_KMS_BLOCKED=QEMU_DMI_TTY1_OPTIN_REQUIRED".into());
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
        "/run/melania-native-qemu/frame-stream.sock"
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(12)))?;
    stream.set_write_timeout(Some(Duration::from_secs(7)))?;
    println!("MELANIA_RUST_STREAM_KMS_CONNECTED=true");

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

    let attempt=(||->Result<(),Box<dyn Error>> {
        let total=if std::env::var("MELANIA_QEMU_REOPEN_FRAME").as_deref()==Ok("YES_QEMU_VM") {6u8} else if std::env::var("MELANIA_QEMU_CLOSE_FRAME").as_deref()==Ok("YES_QEMU_VM") {5u8} else if std::env::var("MELANIA_QEMU_DRAG_FRAME").as_deref()==Ok("YES_QEMU_VM") {4u8} else {3u8};
        for index in 0..total {
            // Bounded payload prevents arbitrary allocation or partial frames.
            let mut size=[0u8;4];
            stream.read_exact(&mut size)?;
            if u32::from_le_bytes(size) as usize != W*H*4 {
                return Err("Wayland live-stream length mismatch".into());
            }
            let mut pixels=vec![0u8;W*H*4];
            stream.read_exact(&mut pixels)?;
            if previous.as_ref().is_some_and(|last|*last==pixels) {
                return Err("identical consecutive client commits".into());
            }
            let framebuffer=if index%2==0 {fb1} else {fb2};
            let target=if index%2==0 {&mut db1} else {&mut db2};
            let pitch=target.pitch() as usize;
            if pitch<W*4 {return Err("invalid dumb buffer pitch".into());}
            {
                let mut map=card.map_dumb_buffer(target)?;
                let bytes=map.as_mut();
                if bytes.len()<H*pitch {
                    return Err("DRM map shorter than image".into());
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
            wait_for_fb(&card,crtc_handle,framebuffer)?;
            println!("MELANIA_RUST_STREAM_FRAME{}_ACTIVE=true",index+1);
            if std::env::var("MELANIA_QEMU_TWO_WINDOWS").as_deref()==Ok("YES_QEMU_VM") {
                println!("MELANIA_RUST_MULTI_FRAME{}_ACTIVE=true",index+1);
            }
            if total>=4 && index==3 {
                println!("MELANIA_RUST_DRAG_FRAME_ACTIVE=true");
            }
            if total>=5 && index==4 {
                println!("MELANIA_RUST_CLOSE_FRAME5_ACTIVE=true");
            }
            if total==6 && index==5 {
                println!("MELANIA_RUST_REOPEN_FRAME6_ACTIVE=true");
            }
            previous=Some(pixels);
            // QMP observes actual virtual display bytes while this FB is live.
            thread::sleep(Duration::from_secs(5));
            // Frame 3 is acknowledged *only after* the CRTC has been restored.
            if index==total-1 {
                rollback.restore()?;
            }
            stream.write_all(&[index+1])?;
            stream.flush()?;
            println!("MELANIA_RUST_STREAM_FRAME{}_ACKED=true",index+1);
        }
        Ok(())
    })();

    let restored=rollback.restore();
    if attempt.is_ok() && restored.is_ok() {
        card.destroy_framebuffer(fb2)?;
        card.destroy_framebuffer(fb1)?;
        card.destroy_dumb_buffer(db2)?;
        card.destroy_dumb_buffer(db1)?;
        println!("MELANIA_RUST_STREAM_THREE_CLIENT_FRAMES_PASS=true");
        if std::env::var("MELANIA_QEMU_DRAG_FRAME").as_deref()==Ok("YES_QEMU_VM") {
            println!("MELANIA_RUST_COMPOSITOR_DRAG_FRAME4_PASS=true");
        }
        if std::env::var("MELANIA_QEMU_CLOSE_FRAME").as_deref()==Ok("YES_QEMU_VM") {
            println!("MELANIA_RUST_CLOSE_FRAME5_PASS=true");
        }
        if std::env::var("MELANIA_QEMU_REOPEN_FRAME").as_deref()==Ok("YES_QEMU_VM") {
            println!("MELANIA_RUST_REOPEN_FRAME6_PASS=true");
        }
        println!("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false");
        println!("MELANIA_NATIVE_SESSION_READY=false");
        return Ok(());
    }
    attempt?;
    restored?;
    Err("QEMU Rust live KMS stream failed".into())
}

fn main() {
    if let Err(error)=run() {
        eprintln!("MELANIA_RUST_STREAM_KMS_FAIL={error}");
        std::process::exit(1);
    }
}
