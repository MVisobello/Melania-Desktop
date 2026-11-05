//! Real GBM/EGL -> DMA-BUF -> DRM/KMS scanout in a disposable QEMU VM.
//!
//! Unlike the offscreen probe, these frames are submitted to the kernel CRTC.
//! They are NOT a Wayland compositor, GBM swapchain, VT-resume implementation,
//! hotplug handler or installable login session. The CLI caller enforces QEMU
//! DMI, a dedicated /dev/tty1 and explicit permission before opening libseat.
//! libseat retains the original descriptor while this function uses a clone.
//!
//! An explicit restoration attempt takes place before any GPU framebuffer is
//! released, even on render/import/flip failure. The Drop guard is a second
//! best-effort recovery path for unexpected early unwinding.
use drm::control::{self, connector, crtc, framebuffer, Device as ControlDevice, PageFlipFlags};
use smithay::backend::{
    allocator::{
        gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        dmabuf::DmabufAllocator,
        Allocator, Fourcc, Modifier,
    },
    drm::{gbm::framebuffer_from_dmabuf, DrmDeviceFd},
    egl::{EGLContext, EGLDisplay},
    renderer::{gles::GlesRenderer, Bind, Color32F, Frame, Renderer},
};
use smithay::utils::{DeviceFd, Rectangle, Transform};
use std::{error::Error, os::fd::OwnedFd, thread, time::{Duration, Instant}};

struct Restore<'a> {
    card: &'a DrmDeviceFd,
    original: crtc::Info,
    connector: connector::Handle,
    submitted: bool,
    restored: bool,
}

impl Restore<'_> {
    fn restore(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.submitted {
            self.restored=true;
            return Ok(());
        }
        let old=self.original;
        self.card.set_crtc(old.handle(),old.framebuffer(),old.position(),
            &[self.connector],old.mode())?;
        let observed=self.card.get_crtc(old.handle())?;
        let attached=self.card.get_connector(self.connector,false)?
            .current_encoder()
            .and_then(|enc|self.card.get_encoder(enc).ok())
            .and_then(|enc|enc.crtc())==Some(old.handle());
        self.restored=observed.framebuffer()==old.framebuffer()
            && observed.position()==old.position()
            && observed.mode()==old.mode() && attached;
        println!("MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE={attached}");
        println!("MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE={}",self.restored);
        if !self.restored {
            return Err("QEMU GPU-generated framebuffer original CRTC restore failed".into());
        }
        Ok(())
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.submitted && !self.restored {
            let _=self.restore();
        }
    }
}

fn wait_for_scanout(
    card:&DrmDeviceFd, crtc:crtc::Handle, connector:connector::Handle,
    expected:framebuffer::Handle,
)->Result<(),Box<dyn Error>> {
    let until=Instant::now()+Duration::from_secs(3);
    while Instant::now()<until {
        if crate::native_kms_scanout::verify_kernel_scanout(
            card,crtc,connector,expected,
        )? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("QEMU GPU-produced DMA-BUF was never observed on actual CRTC".into())
}

/// Performs two synchronized GLES clears in full-size GBM DMA-BUFs and
/// displays them through REAL DRM set_crtc/page_flip kernel ioctls.
///
/// The incoming descriptor MUST be a clone of a live libseat card opened by
/// the QEMU-only entrypoint. No host display or display-manager invocation.
pub fn qemu_two_gpu_frames(card_fd:OwnedFd) -> Result<(),Box<dyn Error>> {
    let card=DrmDeviceFd::new(DeviceFd::from(card_fd));
    let resources=card.resource_handles()?;
    if resources.crtcs().len()!=1 || resources.connectors().is_empty() {
        return Err("QEMU GBM scanout requires exactly one active CRTC".into());
    }
    let topology=crate::native_output_topology::inspect_stable_active_outputs(&card)?;
    let route=topology.single_active()?;
    println!("MELANIA_NATIVE_GBM_KMS_TOPOLOGY_STABLE_OUTPUTS={}",topology.active.len());
    let crtc=route.crtc;
    let original=card.get_crtc(crtc)?;
    let mode=original.mode().ok_or("QEMU GPU scanout requires an active CRTC mode")?;
    if original.framebuffer().is_none() {
        return Err("QEMU GPU scanout requires a restorable original FB".into());
    }
    let (w,h)=mode.size();
    crate::renderer::validate_gbm_output_dimensions(u32::from(w),u32::from(h),2)?;
    if original.framebuffer()!=Some(route.original_fb) {
        return Err("QEMU GBM original FB_ID changed after topology capture".into());
    }
    let connector=route.connector;

    // Use SCANOUT as well as RENDERING; a renderable-only BO must not be
    // misrepresented as suitable for DRM KMS.
    let gbm=GbmDevice::new(card.clone())?;
    let mut allocator=DmabufAllocator(GbmAllocator::new(
        gbm.clone(),GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    ));
    let mut a=allocator.create_buffer(u32::from(w),u32::from(h),
        Fourcc::Argb8888,&[Modifier::Linear])?;
    let mut b=allocator.create_buffer(u32::from(w),u32::from(h),
        Fourcc::Argb8888,&[Modifier::Linear])?;
    // These objects own real kernel FB_ID lifetimes. They must outlive the
    // visible scanout, and are dropped only AFTER restoring the old CRTC.
    let fb_a=framebuffer_from_dmabuf(&card,&gbm,&a,true,true)?;
    let fb_b=framebuffer_from_dmabuf(&card,&gbm,&b,true,true)?;
    let handles=[*fb_a.as_ref(),*fb_b.as_ref()];

    let display=unsafe {EGLDisplay::new(gbm)}?;
    let context=EGLContext::new(&display)?;
    let mut renderer=unsafe {GlesRenderer::new(context)}?;
    let mut flip_ledger=crate::native_flip_ledger::NativeFlipLedger::default();
    let mut event_router=crate::native_drm_flip::DrmFlipEventRouter::default();
    let mut rollback=Restore {
        card:&card, original, connector, submitted:false, restored:false,
    };
    let work=(||->Result<(),Box<dyn Error>> {
        for index in 0..2 {
            let buffer=if index==0 {&mut a} else {&mut b};
            let mut framebuffer=renderer.bind(buffer)?;
            let mut frame=renderer.render(
                &mut framebuffer,(i32::from(w),i32::from(h)).into(),
                Transform::Normal,
            )?;
            let color=if index==0 {
                Color32F::new(0.07,0.28,0.67,1.0)
            } else {
                Color32F::new(0.20,0.65,0.23,1.0)
            };
            frame.clear(color,&[Rectangle::from_size(
                (i32::from(w),i32::from(h)).into(),
            )])?;
            frame.finish()?.wait()?;
            drop(framebuffer);

            if index==0 {
                // Even a failed initial modeset requires restoration.
                rollback.submitted=true;
                card.set_crtc(crtc,Some(handles[index]),original.position(),
                    &connected,Some(mode))?;
            } else {
                // EAGAIN means the prior asynchronous flip has not retired.
                // Only that error is retryable and the deadline is bounded.
                flip_ledger.begin_flip()
                    .map_err(|error|format!("GBM flip already pending or revoked: {error:?}"))?;
                let until=Instant::now()+Duration::from_secs(3);
                loop {
                    match card.page_flip(crtc,handles[index],PageFlipFlags::EVENT,None) {
                        Ok(())=>break,
                        Err(error) if error.raw_os_error()==Some(11) && Instant::now()<until =>
                            thread::sleep(Duration::from_millis(20)),
                        Err(error)=>return Err(error.into()),
                    }
                }
            }
            if index>0 {
                let kernel_frame=crate::native_drm_flip::wait_for_kms_page_flip_routed(
                    &card,crtc,Duration::from_secs(3),&mut event_router,
                )?;
                flip_ledger.complete_vm_flip(kernel_frame)
                    .map_err(|error|format!("GBM kernel flip event rejected: {error:?}"))?;
                if flip_ledger.unreliable_vm_counter() {
                    println!("MELANIA_NATIVE_GBM_KMS_VBLANK_COUNTER_UNRELIABLE_QEMU=true");
                }
                println!("MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK={}",index+1);
            }
            wait_for_scanout(&card,crtc,connector,handles[index])?;
            if index>0 {
                println!("MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED={}",
                         flip_ledger.completed_count());
            }
            println!("MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE={}",index+1);
            // Keep a visible interval for an isolated future QMP pixel check.
            thread::sleep(Duration::from_millis(450));
        }
        Ok(())
    })();
    // IMPORTANT: restoration must precede dropping the GBM DMA-BUFs and
    // GbmFramebuffer kernel FB_ID handles, also when work failed.
    let restore=rollback.restore();
    work?;
    restore?;
    println!("MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true");
    Ok(())
}
