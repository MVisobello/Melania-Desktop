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
    path::Path,
    thread,
    time::{Duration,Instant},
};

const W:usize=1280;
const H:usize=800;
const FIRST:&str="/run/melania-native-qemu/client-frame.bgra";
const SECOND:&str="/run/melania-native-qemu/client-frame-2.bgra";
const OPT_IN:&str="MELANIA_QEMU_RUST_TWO_FRAME_KMS";

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
        return Err("MELANIA_RUST_KMS_BLOCKED=QEMU_DMI_TTY1_OPTIN_REQUIRED".into());
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
    let first=read_image(FIRST)?;
    let second=read_image(SECOND)?;
    if first==second {
        return Err("two Wayland client commits have identical pixels".into());
    }
    let card=Card(OpenOptions::new().read(true).write(true).open("/dev/dri/card0")?);
    let resources=card.resource_handles()?;
    if resources.crtcs().len()!=1 || resources.connectors().is_empty() ||
       resources.connectors().len()>8 {
        return Err("QEMU test requires exactly one CRTC and at most eight connectors".into());
    }
    let crtc_handle=resources.crtcs()[0];
    let original=card.get_crtc(crtc_handle)?;
    let old_mode=original.mode().ok_or("QEMU original CRTC has no active mode")?;
    if old_mode.size()!=(W as u16,H as u16) || original.framebuffer().is_none() {
        return Err("QEMU original active framebuffer must be 1280x800".into());
    }
    let connected=resources.connectors().iter().filter_map(|&conn|{
        let info=card.get_connector(conn,false).ok()?;
        let encoder=info.current_encoder()?;
        let enc_info=card.get_encoder(encoder).ok()?;
        (enc_info.crtc()==Some(crtc_handle)).then_some(conn)
    }).collect::<Vec<_>>();
    if connected.len()!=1 {
        return Err("QEMU requires exactly one active connector".into());
    }

    let mut db1=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    let mut db2=card.create_dumb_buffer((W as u32,H as u32),DrmFourcc::Xrgb8888,32)?;
    for (buf,pixels) in [(&mut db1,&first),(&mut db2,&second)] {
        let pitch=buf.pitch() as usize;
        if pitch<W*4 {return Err("invalid DRM pitch".into());}
        let mut map=card.map_dumb_buffer(buf)?;
        let dst=map.as_mut();
        if dst.len()<H*pitch {return Err("DRM dumb mapping incomplete".into());}
        for y in 0..H {
            dst[y*pitch..y*pitch+W*4].copy_from_slice(&pixels[y*W*4..(y+1)*W*4]);
        }
    }
    let fb1=card.add_framebuffer(&db1,24,32)?;
    let fb2=card.add_framebuffer(&db2,24,32)?;
    let mut rollback=Restore {
        card:&card,original,connector:connected[0],submitted:false,restored:false
    };

    let attempt=(||->Result<(),Box<dyn Error>>{
        // Set the original mode with the new client-derived buffer; unlike
        // Stage26, this is Rust drm-rs, not a separate C modeset process.
        card.set_crtc(crtc_handle,Some(fb1),original.position(),&connected,Some(old_mode))?;
        rollback.submitted=true;
        wait_for_fb(&card,crtc_handle,fb1)?;
        println!("MELANIA_RUST_KMS_FRAME1_ACTIVE=true");
        // The QMP side captures both snapshots independently. Bounded holds
        // are intentional so CI can prove pixels on the VM display.
        thread::sleep(Duration::from_secs(7));

        card.page_flip(crtc_handle,fb2,PageFlipFlags::empty(),None)?;
        wait_for_fb(&card,crtc_handle,fb2)?;
        println!("MELANIA_RUST_KMS_FRAME2_ACTIVE=true");
        thread::sleep(Duration::from_secs(7));
        Ok(())
    })();

    let restore=rollback.restore();
    if attempt.is_ok() && restore.is_ok() {
        // Only release resources after the original FB_ID has been observed
        // again. This does not certify rollback after SIGKILL or a GPU reset.
        card.destroy_framebuffer(fb2)?;
        card.destroy_framebuffer(fb1)?;
        card.destroy_dumb_buffer(db2)?;
        card.destroy_dumb_buffer(db1)?;
        println!("MELANIA_RUST_KMS_TWO_CLIENT_FRAMES_PASS=true");
        println!("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false");
        println!("MELANIA_NATIVE_SESSION_READY=false");
        return Ok(());
    }
    attempt?;
    restore?;
    Err("Rust native KMS attempt failed".into())
}

fn main() {
    if let Err(error)=run() {
        eprintln!("MELANIA_RUST_KMS_TWO_CLIENT_FRAMES_FAIL={error}");
        std::process::exit(1);
    }
}
