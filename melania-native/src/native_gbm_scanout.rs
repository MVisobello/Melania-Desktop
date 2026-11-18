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
use drm::control::{self, connector, crtc, encoder, framebuffer, Device as ControlDevice, PageFlipFlags};
use smithay::backend::{
    allocator::{
        gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        dmabuf::DmabufAllocator,
        Allocator, Fourcc, Modifier,
    },
    drm::{gbm::framebuffer_from_dmabuf, DrmDeviceFd},
    egl::{EGLContext, EGLDisplay},
    renderer::{gles::GlesRenderer, Bind, Color32F, Frame, ImportMem, Renderer},
};
use smithay::utils::{DeviceFd, Rectangle, Transform};
use std::{
    error::Error, io::{Read,Write}, os::{fd::OwnedFd,unix::net::UnixStream},
    thread, time::{Duration, Instant},
};

/// Upload a bounded, owned SHM source on the same GLES renderer used
/// by the retained scene. This is a CPU-to-GPU upload, not DMA-BUF
/// readback, and is NOT a presentation receipt or a frame callback.
/// Caller must still sample/retain, finish GPU work, verify the kernel
/// scanout route, and match its Wayland receipt before notifying a client.
/// This primitive alone does not enable the mixed protocol/scene path.
pub fn upload_retained_shm_texture(
    renderer:&mut GlesRenderer,source:&crate::owned_shm::RetainedShmPixels,
)->Result<smithay::backend::renderer::gles::GlesTexture,Box<dyn Error>> {
    use smithay::backend::renderer::Texture;
    let (width,height)=source.dimensions();
    let texture=renderer.import_memory(source.bytes(),source.format(),(width,height).into(),false)?;
    if (texture.width() as i32,texture.height() as i32)!=(width,height) {
        return Err("retained SHM upload texture dimensions differ from source".into());
    }
    Ok(texture)
}

struct Restore<'a> {
    card: &'a DrmDeviceFd,
    original: crtc::Info,
    connector: connector::Handle,
    original_encoder: encoder::Handle,
    submitted: bool,
    restored: bool,
    ownership_lost: bool,
    owned_fbs:[framebuffer::Handle;2],
}

impl Restore<'_> {
    fn restore(&mut self) -> Result<(),Box<dyn Error>> {
        if self.ownership_lost {
            return Err("DRM ownership lost: refusing stale CRTC rollback".into());
        }
        if !self.submitted {
            self.restored=true;
            return Ok(());
        }
        let old=self.original;
        // Do not overwrite a foreign scanout acquired since the last ACK.
        // An unreadable DRM resource is a lost ownership proof. Mark the
        // rollback guard permanently unsafe, including its Drop retry.
        // Refuse rollback if connector, original encoder, full mode, CRTC
        // position or owned FB_ID changed, even when the CRTC ID is unchanged.
        let safe=crate::native_kms_scanout::verify_rollback_route(
            self.card,&old,self.connector,self.original_encoder,&self.owned_fbs,
        ).map_err(|error|{
            self.ownership_lost=true;
            error
        })?;
        if !safe {
            self.ownership_lost=true;
            return Err("DRM rollback refused: original connector/encoder/mode/position/framebuffer changed".into());
        }
        println!("MELANIA_NATIVE_GBM_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true");
        // A failed restoring modeset can have partially affected kernel state.
        // Never let Drop issue the same ioctl again without fresh ownership
        // acquisition: the original route may have been taken over.
        self.card.set_crtc(old.handle(),old.framebuffer(),old.position(),
            &[self.connector],old.mode()).map_err(|error|{
            self.ownership_lost=true;
            error
        })?;
        // Once a restore ioctl has been submitted, unreadable verification
        // cannot authorize Drop to issue the same modeset again.
        let attached=crate::native_kms_scanout::verify_rollback_route(
            self.card,&old,self.connector,self.original_encoder,&[],
        ).map_err(|error|{
            self.ownership_lost=true;
            error
        })?;
        self.restored=attached;
        println!("MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE={attached}");
        println!("MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE={}",self.restored);
        if !self.restored {
            // Unknown post-restore state is not proof of continued DRM
            // ownership: Drop must not blindly repeat the modeset.
            self.ownership_lost=true;
            return Err("QEMU GPU-generated framebuffer original CRTC restore failed".into());
        }
        Ok(())
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.submitted && !self.restored && !self.ownership_lost {
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

/// Preserve the original two-frame smoke exactly as a compatibility gate.
/// The same actual KMS path also supports a longer, explicitly QEMU-only
/// GPU-backed render loop, reusing both real DMA-BUF allocations.
pub fn qemu_two_gpu_frames(card_fd:OwnedFd) -> Result<(),Box<dyn Error>> {
    qemu_gpu_frames(card_fd,2)
}

/// Drive multiple real GLES render + CRTC pageflips on two retained DMA-BUFs.
/// This remains a disposable VM GPU stress probe, not a Wayland renderer.
pub fn qemu_stress_gpu_frames(card_fd:OwnedFd,frames:u32)
    ->Result<(),Box<dyn Error>>
{
    if frames!=24 {
        return Err("GBM stress run requires the isolated 24-frame QEMU contract".into());
    }
    qemu_gpu_frames(card_fd,frames)
}

fn qemu_gpu_frames(card_fd:OwnedFd,total_frames:u32)
    ->Result<(),Box<dyn Error>>
{
    if !(2..=24).contains(&total_frames) {
        return Err("GBM KMS frame count exceeds isolated QEMU limits".into());
    }
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
    let original_encoder=card.get_connector(connector,false)?
        .current_encoder().ok_or("Native GBM output missing its original connector encoder")?;
    // The stable DRM snapshot identified THIS encoder, not merely its
    // destination CRTC. A VT/hotplug takeover can swap the encoder while
    // keeping the same CRTC and framebuffer: do not adopt that new route.
    // Both the 24-frame and persistent GPU Wayland paths must check before
    // creating or reusing any GBM scanout buffers.
    if original_encoder!=route.original_encoder {
        return Err("GBM KMS original encoder changed since stable topology".into());
    }
    if card.get_encoder(original_encoder)?.crtc()!=Some(crtc) {
        return Err("Native GBM encoder route changed after topology acquisition".into());
    }
    // drm-rs set_crtc expects a connector slice; retain exactly the
    // verified connector from this stable kernel topology snapshot.
    let connected=[connector];

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
    event_router.bind_active_crtcs(&[crtc])?;
    let mut rollback=Restore {
        card:&card, original, connector, original_encoder, submitted:false, restored:false, ownership_lost:false,owned_fbs:handles,
    };
    let work=(||->Result<(),Box<dyn Error>> {
        for index in 0..total_frames {
            // Never render into the currently visible buffer: the preceding
            // flip's DRM event AND actual scanout verification have completed.
            // Two GPU DMA-BUFs are reused without per-frame allocations.
            let buffer=if index%2==0 {&mut a} else {&mut b};
            let mut framebuffer=renderer.bind(buffer)?;
            let mut frame=renderer.render(
                &mut framebuffer,(i32::from(w),i32::from(h)).into(),
                Transform::Normal,
            )?;
            let color=if index%2==0 {
                Color32F::new(0.07,0.28,0.67,1.0)
            } else {
                Color32F::new(0.20,0.65,0.23,1.0)
            };
            frame.clear(color,&[Rectangle::from_size(
                (i32::from(w),i32::from(h)).into(),
            )])?;
            frame.finish()?.wait()?;
            drop(framebuffer);

            let prior_fb=if index==0 {route.original_fb}
                else {handles[(index-1) as usize%2]};
            // Even a failed DRM ioctl read is loss of trusted ownership.
            // Revoke the pending event epoch before propagating the error.
            let owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                &card,crtc,connector,original_encoder,prior_fb,(w,h),
            mode,original.position()).map_err(|error| {
                event_router.revoke();
                rollback.ownership_lost=true;
                error
            })?;
            if !owned {
                event_router.revoke();
                rollback.ownership_lost=true;
                return Err("GPU stress scanout owner changed before KMS submission".into());
            }
            if index==0 {
                // Even a failed initial modeset requires restoration.
                rollback.submitted=true;
                card.set_crtc(crtc,Some(handles[index as usize%2]),original.position(),
                    &connected,Some(mode))?;
            } else {
                // EAGAIN means the prior asynchronous flip has not retired.
                // Only that error is retryable and the deadline is bounded.
                flip_ledger.begin_flip()
                    .map_err(|error|format!("GBM flip already pending or revoked: {error:?}"))?;
                let until=Instant::now()+Duration::from_secs(3);
                loop {
                    // Verify the real encoder/CRTC/FB before EVERY ioctl.
                    // EAGAIN followed by a VT change must never turn the
                    // next attempt into a page flip on somebody else's CRTC.
                    let retry_owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                        &card,crtc,connector,original_encoder,prior_fb,(w,h),
                    mode,original.position()).map_err(|error|{
                        event_router.revoke();
                        rollback.ownership_lost=true;
                        error
                    })?;
                    if !retry_owned {
                        event_router.revoke();
                        rollback.ownership_lost=true;
                        return Err("GBM stress page-flip retry lost original DRM ownership".into());
                    }
                    match card.page_flip(crtc,handles[index as usize%2],PageFlipFlags::EVENT,None) {
                        Ok(())=>{
                            event_router.arm_flip(crtc)?;
                            break;
                        },
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
            wait_for_scanout(&card,crtc,connector,handles[index as usize%2])?;
            // A DRM vblank and FB_ID do not authorize a frame ACK if the
            // original connector encoder has changed on this live CRTC.
            let ack_owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                &card,crtc,connector,original_encoder,handles[index as usize%2],(w,h),
            mode,original.position()).map_err(|error|{
                event_router.revoke();
                rollback.ownership_lost=true;
                error
            })?;
            if !ack_owned {
                event_router.revoke();
                rollback.ownership_lost=true;
                return Err("GPU KMS original encoder changed before frame ACK".into());
            }
            println!("MELANIA_NATIVE_GBM_KMS_ORIGINAL_ENCODER_FRAME_ACK={}",index+1);
            if index>0 {
                println!("MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED={}",
                         flip_ledger.completed_count());
            }
            println!("MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE={}",index+1);
            // Keep a visible interval for an isolated future QMP pixel check.
            thread::sleep(Duration::from_millis(if total_frames==2 {450}else{75}));
        }
        Ok(())
    })();
    // IMPORTANT: restoration must precede dropping the GBM DMA-BUFs and
    // GbmFramebuffer kernel FB_ID handles, also when work failed.
    let restore=rollback.restore();
    work?;
    restore?;
    if total_frames==2 {
        println!("MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true");
    } else {
        println!("MELANIA_NATIVE_GBM_KMS_REUSED_DMA_BUFS=2");
        println!("MELANIA_NATIVE_GBM_KMS_24_GPU_FRAMES_PASS=true");
    }
    Ok(())
}


/// Stage109: a REAL Wayland scene drawn on the GPU and submitted to KMS.
///
/// The input is the same versioned 1280x800 owned wl_shm compositor scene
/// produced by wayland_bridge.rs for Stage105. The only change is that each
/// authenticated scene snapshot is uploaded via Smithay ImportMem to a GLES
/// texture, rendered into alternating genuine GBM scanout DMA-BUFs, then
/// submitted to an actual VirtIO DRM CRTC. No synthetic color frames or
/// screenshot files enter this path. libseat owns the original descriptor.
///
/// This remains an EXPLICIT 24-frame QEMU checkpoint. The Wayland server
/// gates the libseat session, DMI and dedicated TTY before calling it.
/// Nothing here registers a production login or enables native readiness.
pub fn qemu_wayland_gpu_scene_stream(
    stream:UnixStream, card_fd:OwnedFd,
)->Result<(),Box<dyn Error>> {
    qemu_gpu_scene_stream(stream,card_fd,None)
}

pub fn qemu_direct_client_gpu_scene_stream(
    stream:UnixStream,card_fd:OwnedFd,
    gpu:std::sync::mpsc::Receiver<crate::continuous_kms_stream::DirectGpuFrame>,
)->Result<(),Box<dyn Error>> {
    qemu_gpu_scene_stream(stream,card_fd,Some(gpu))
}

// Verify a few real rendered GBM pixels for the VM acceptance test only.
// Readback is diagnostic; it never feeds rendering or KMS framebuffer data.
// The VirtIO guest crashed inside Smithay finish on the second frame even
// with ExportFence disabled and every client image retained. Avoid diagnostic
// PBOs entirely: their delayed unmap/delete is processed inside the next
// GlesFrame::finish. Read one pixel synchronously from the CURRENT real
// render target; this is diagnostic only and never provides scene pixels.
// The caller must have just finished a frame on the same EGL context/FBO.
fn sample_direct_gpu_pixel(renderer:&mut GlesRenderer,x:i32,y:i32)
    ->Result<[u8;3],Box<dyn Error>> {
    use smithay::backend::renderer::gles::ffi;
    let pixel=renderer.with_context(|gl|->Result<[u8;4],String>{
        let mut fbo=0;
        let mut pack_buffer=0;
        let mut read_buffer=0;
        let mut rgba=[0u8;4];
        unsafe {
            gl.GetIntegerv(ffi::READ_FRAMEBUFFER_BINDING,&mut fbo);
            if fbo==0 || gl.CheckFramebufferStatus(ffi::READ_FRAMEBUFFER)!=ffi::FRAMEBUFFER_COMPLETE {
                return Err("direct GPU diagnostic has no complete bound framebuffer".into());
            }
            gl.GetIntegerv(ffi::PIXEL_PACK_BUFFER_BINDING,&mut pack_buffer);
            gl.GetIntegerv(ffi::READ_BUFFER,&mut read_buffer);
            // Restore all state modified by this tiny diagnostic. No GPU
            // mapping object or deferred PBO destruction survives the call.
            gl.BindBuffer(ffi::PIXEL_PACK_BUFFER,0);
            gl.ReadBuffer(ffi::COLOR_ATTACHMENT0);
            gl.ReadPixels(x,y,1,1,ffi::RGBA,ffi::UNSIGNED_BYTE,rgba.as_mut_ptr().cast());
            let error=gl.GetError();
            gl.ReadBuffer(read_buffer as u32);
            gl.BindBuffer(ffi::PIXEL_PACK_BUFFER,pack_buffer as u32);
            if error!=ffi::NO_ERROR {return Err(format!("direct GPU diagnostic glReadPixels error {error:#x}"));}
        }
        Ok(rgba)
    })??;
    Ok([pixel[0],pixel[1],pixel[2]])
}

// Normalize the opaque client EGL image into a compositor-owned RGBA
// texture using a GPU framebuffer blit, not a CPU download/upload. The
// focused VM reproduced a crash during glFinish when sampling the second
// imported XRGB texture directly. Keep ARGB sampling and its premultiplied
// alpha test unchanged. This is a proposed driver-path workaround.
fn normalize_opaque_direct_source(
    renderer:&mut GlesRenderer,
    source:&mut smithay::backend::renderer::gles::GlesTexture,
)->Result<smithay::backend::renderer::gles::GlesTexture,Box<dyn Error>> {
    use smithay::backend::renderer::{Blit,Offscreen,Texture,TextureFilter};
    use smithay::backend::renderer::gles::{ffi,GlesTexture};
    let size=(source.width() as i32,source.height() as i32);
    let mut copy:GlesTexture=renderer.create_buffer(Fourcc::Abgr8888,size.into())?;
    {
        let input=renderer.bind(source)?;
        let mut output=renderer.bind(&mut copy)?;
        let rect=Rectangle::from_size(size.into());
        renderer.blit(&input,&mut output,rect,rect,TextureFilter::Nearest)?;
        // The undefined X byte is never an alpha channel. Force alpha=1
        // while preserving the GPU-copied RGB channels. Restore GL state.
        renderer.with_context(|gl|->Result<(),String>{
            unsafe {
                if gl.CheckFramebufferStatus(ffi::DRAW_FRAMEBUFFER)!=ffi::FRAMEBUFFER_COMPLETE {
                    return Err("opaque GPU normalization framebuffer incomplete".into());
                }
                let mut mask=[0u8;4];
                let mut clear=[0f32;4];
                gl.GetBooleanv(ffi::COLOR_WRITEMASK,mask.as_mut_ptr());
                gl.GetFloatv(ffi::COLOR_CLEAR_VALUE,clear.as_mut_ptr());
                let scissor=gl.IsEnabled(ffi::SCISSOR_TEST)==ffi::TRUE;
                gl.Disable(ffi::SCISSOR_TEST);
                gl.ColorMask(ffi::FALSE,ffi::FALSE,ffi::FALSE,ffi::TRUE);
                gl.ClearColor(0.0,0.0,0.0,1.0);
                gl.Clear(ffi::COLOR_BUFFER_BIT);
                gl.ColorMask(mask[0],mask[1],mask[2],mask[3]);
                gl.ClearColor(clear[0],clear[1],clear[2],clear[3]);
                if scissor {gl.Enable(ffi::SCISSOR_TEST);}
                gl.Finish();
                let error=gl.GetError();
                if error!=ffi::NO_ERROR {
                    return Err(format!("opaque GPU normalization failed {error:#x}"));
                }
            }
            Ok(())
        })??;
    }
    Ok(copy)
}

// An opaque window needs no blend shader. The Mesa llvmpipe stack trace
// crashed in its rasterizer even after XRGB normalization succeeded.
// Copy the opaque image into the actual scanout FBO with a GLES3 blit;
// preserve the same vertical transform as the alpha-composition path.
fn blit_opaque_direct_window(
    renderer:&mut GlesRenderer,
    source:&mut smithay::backend::renderer::gles::GlesTexture,
    (x,y):(i32,i32),
)->Result<(),Box<dyn Error>> {
    use smithay::backend::renderer::Texture;
    use smithay::backend::renderer::gles::ffi;
    let draw=renderer.with_context(|gl|{
        let mut id=0;
        unsafe {gl.GetIntegerv(ffi::DRAW_FRAMEBUFFER_BINDING,&mut id);}
        id
    })?;
    if draw==0 {return Err("opaque direct scanout FBO missing".into());}
    let (w,h)=(source.width() as i32,source.height() as i32);
    {
        let mut input=renderer.bind(source)?;
        // Obtain the actual source FBO via Smithay's binding, not a
        // guessed GL object name. Rendering an empty frame changes no pixels.
        let mut frame=renderer.render(&mut input,(w,h).into(),Transform::Normal)?;
        let read=frame.with_context(|gl|{
            let mut id=0;
            unsafe {gl.GetIntegerv(ffi::READ_FRAMEBUFFER_BINDING,&mut id);}
            id
        })?;
        frame.finish()?.wait()?;
        if read==0 || read==draw {return Err("opaque direct source FBO invalid".into());}
        renderer.with_context(|gl|->Result<(),String>{
            unsafe {
                gl.BindFramebuffer(ffi::READ_FRAMEBUFFER,read as u32);
                gl.BindFramebuffer(ffi::DRAW_FRAMEBUFFER,draw as u32);
                if gl.CheckFramebufferStatus(ffi::READ_FRAMEBUFFER)!=ffi::FRAMEBUFFER_COMPLETE ||
                   gl.CheckFramebufferStatus(ffi::DRAW_FRAMEBUFFER)!=ffi::FRAMEBUFFER_COMPLETE {
                    return Err("opaque direct blit framebuffer incomplete".into());
                }
                let scissor=gl.IsEnabled(ffi::SCISSOR_TEST)==ffi::TRUE;
                gl.Disable(ffi::SCISSOR_TEST);
                gl.ReadBuffer(ffi::COLOR_ATTACHMENT0);
                gl.BlitFramebuffer(0,h,w,0,x,y,x+w,y+h,
                    ffi::COLOR_BUFFER_BIT,ffi::NEAREST);
                gl.Finish();
                let error=gl.GetError();
                if scissor {gl.Enable(ffi::SCISSOR_TEST);}
                // Subsequent pixel verification must read the real output,
                // not the source. Restore the output as both GL targets.
                gl.BindFramebuffer(ffi::FRAMEBUFFER,draw as u32);
                if error!=ffi::NO_ERROR {
                    return Err(format!("opaque direct scanout blit error {error:#x}"));
                }
            }
            Ok(())
        })??;
    }
    Ok(())
}

fn verify_direct_client_gpu_pixels(
    renderer:&mut GlesRenderer,
    bound:&smithay::backend::renderer::gles::GlesTarget<'_>,
    index:u64,
)->Result<(),Box<dyn Error>> {
    fn sample(renderer:&mut GlesRenderer,
        _bound:&smithay::backend::renderer::gles::GlesTarget<'_>,
        x:i32,y:i32)->Result<[u8;3],Box<dyn Error>> {
        sample_direct_gpu_pixel(renderer,x,y)
    }
    fn close(actual:[u8;3],expected:[u8;3],tolerance:u8)->bool {
        actual.into_iter().zip(expected).all(|(a,b)|a.abs_diff(b)<=tolerance)
    }
    let center=sample(renderer,bound,340,232)?;
    let expected=if index==0 {[84,46,186]} else {[31,163,69]};
    if !close(center,expected,20) {
        return Err(format!("direct client GBM center mismatch: {center:?} expected {expected:?}").into());
    }
    if index==0 {
        let opaque=sample(renderer,bound,200,160)?;
        let alpha=sample(renderer,bound,200,303)?;
        if !close(opaque,expected,20) || !close(alpha,[56,39,122],13) {
            return Err(format!("direct GBM alpha/orientation mismatch: {opaque:?} {alpha:?}").into());
        }
    } else {
        let top=sample(renderer,bound,200,160)?;
        let bottom=sample(renderer,bound,200,303)?;
        if !close(top,expected,20) || !close(bottom,[201,61,20],5) {
            return Err(format!("opaque GPU blit orientation mismatch: {top:?} {bottom:?}").into());
        }
    }
    let background=sample(renderer,bound,5,5)?;
    let border=sample(renderer,bound,157,120)?;
    if !close(background,[27,33,58],3) || !close(border,[65,171,216],3) {
        return Err(format!("direct GBM desktop geometry mismatch: {background:?} {border:?}").into());
    }
    Ok(())
}

fn verify_retained_gpu_scene(
    renderer:&mut GlesRenderer,
    bound:&smithay::backend::renderer::gles::GlesTarget<'_>,
    index:u64,removed_slot:Option<u64>,moved_slot:Option<(u64,i32,i32)>,
    visibility_slot:Option<(u64,bool)>,raised_slot:Option<u64>,layout:&crate::retained_gpu_layout::RetainedGpuLayout,
)->Result<(),Box<dyn Error>> {
    let _=bound;
    use crate::window_placement::{retained_gpu_geometry,retained_gpu_probe_size,
        retained_gpu_probe_rgb,RETAINED_GPU_PROBE_SLOTS};
    let mut expected_by_slot=std::collections::BTreeMap::new();
    let mut previous_size=None;
    for (frame,&slot) in RETAINED_GPU_PROBE_SLOTS.iter().take(index as usize+1).enumerate() {
        let previous=expected_by_slot.insert(slot,frame as u64);
        if frame as u64==index {previous_size=previous.and_then(retained_gpu_probe_size);}
    }
    let sample=|renderer:&mut GlesRenderer,x:i32,y:i32,expected:[u8;3]|->Result<(),Box<dyn Error>> {
        let rgba=sample_direct_gpu_pixel(renderer,x,y)?;
        if rgba.len()<3 || !rgba[..3].iter().zip(expected).all(|(actual,want)|
            actual.abs_diff(want)<=4) {
            return Err(format!("retained GPU scene frame {index} pixel ({x},{y}) mismatch: {rgba:?} expected {expected:?}").into());
        }
        Ok(())
    };
    if index>=12 {
        if index==12 && removed_slot!=Some(2) {return Err("expected actual destroyed GPU surface 2".into());}
        let prior=expected_by_slot.remove(&2).ok_or("missing closed GPU window")?;
        let (w,h)=retained_gpu_probe_size(prior).ok_or("missing closed GPU extent")?;
        let (x,y,w,h)=retained_gpu_geometry(2,w,h).ok_or("missing closed GPU geometry")?;
        if index==12 {
            for (px,py) in [(x+w/2,y+h/2),(x+w-3,y+h-3),(x+w+2,y+h/2),(x-2,y+h/2)] {
                sample(renderer,px,py,[27,33,58])?;
            }
        }
        if index==12 {println!("MELANIA_NATIVE_RETAINED_GPU_CLOSE_PIXELS_VERIFIED=2");}
    } else if removed_slot.is_some() {
        return Err("unexpected GPU removal during original 12-frame acceptance".into());
    }
    let live_drag=matches!(std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref(),Ok("LIVE_QEMU_VM"|"VISIBILITY_QEMU_VM"|"STACK_QEMU_VM"|"RESIZE_QEMU_VM"|"LIVE_RESIZE_QEMU_VM"));
    if (13..=14).contains(&index) {
        let (slot,x,y)=moved_slot.ok_or("missing real GPU drag")?;
        let (expected_x,expected_y)=if live_drag && index==13 {(410,155)} else {(650,190)};
        if slot!=1 || (x-expected_x).abs()>3 || (y-expected_y).abs()>3 || removed_slot.is_some() {
            return Err("unexpected real GPU drag destination".into());
        }
        // Old A content and decoration must disappear, not just move a cursor.
        for (px,py) in [(200,150),(360,240),(158,200)] {
            sample(renderer,px,py,[27,33,58])?;
        }
        // The second position must clear the intermediate footprint too.
        if live_drag && index==14 {sample(renderer,460,200,[27,33,58])?;}
    } else if index!=17 && moved_slot.is_some() {return Err("unexpected GPU motion".into());}
    if (15..=16).contains(&index) {
        let visible=index==16;
        if visibility_slot!=Some((1,visible)) || layout.is_visible(1)!=Some(visible) || removed_slot.is_some() {
            return Err("unexpected GPU visibility operation".into());
        }
        let rect=layout.geometry(1).ok_or("hidden GPU role lost geometry")?;
        if (rect.x-650).abs()>3 || (rect.y-190).abs()>3 || (rect.width,rect.height)!=(416,250) {
            return Err("visibility operation changed window geometry".into());
        }
        if !visible {
            for (px,py) in [(rect.x+rect.width/2,rect.y+rect.height/2),(rect.x+rect.width-3,rect.y+rect.height-3),(rect.x-2,rect.y+rect.height/2),(rect.x+rect.width+2,rect.y+rect.height/2)] {
                sample(renderer,px,py,[27,33,58])?;
            }
            expected_by_slot.remove(&1);
        }
        for (px,py) in [(200,150),(360,240),(158,200),(460,200)] {sample(renderer,px,py,[27,33,58])?;}
    } else if visibility_slot.is_some() {return Err("unexpected visibility operation".into());}
    let live_resize=std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("LIVE_RESIZE_QEMU_VM");
    if (index==19 && (live_resize || std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("RESIZE_QEMU_VM"))) ||
        (index==20 && live_resize) {
        let intermediate=live_resize && index==19;
        let (expected_w,expected_h)=if intermediate {(446,268)} else {(466,280)};
        let expected_color=if intermediate {[51,153,204]} else {[204,102,51]};
        let rect=layout.geometry(1).ok_or("missing resized GPU role")?;
        if removed_slot.is_some() || moved_slot.is_some() || visibility_slot.is_some() || raised_slot.is_some() ||
            (rect.x-650).abs()>6 || (rect.y-350).abs()>6 ||
            (rect.width-expected_w).abs()>3 || (rect.height-expected_h).abs()>3 ||
            layout.hit_test(1090.0,400.0)!=Some(1) {
            return Err("GPU resize geometry or operation mismatch".into());
        }
        sample(renderer,700,400,expected_color)?;
        sample(renderer,1090,400,expected_color)?;
        if intermediate {
            sample(renderer,1105,400,[27,33,58])?;
        } else {
            sample(renderer,1105,400,expected_color)?;
        }
        sample(renderer,800,650,retained_gpu_probe_rgb(11).ok_or("missing D color")?)?;
        sample(renderer,300,500,retained_gpu_probe_rgb(9).ok_or("missing C color")?)?;
        sample(renderer,1150,400,[27,33,58])?;
        sample(renderer,rect.x+rect.width+2,400,[65,171,216])?;
        if index==19 {println!("MELANIA_NATIVE_GPU_XDG_RESIZE_PIXELS_VERIFIED=20");}
        else {println!("MELANIA_NATIVE_GPU_XDG_RESIZE_PIXELS_VERIFIED=21");}
        return Ok(());
    }
    if index>=17 {
        if removed_slot.is_some() || visibility_slot.is_some() ||
            layout.is_visible(1)!=Some(true) || layout.is_visible(4)!=Some(true) {
            return Err("stale or hidden overlapping GPU surface".into());
        }
        let rect=layout.geometry(1).ok_or("missing overlapping GPU surface")?;
        if (rect.x-650).abs()>6 || (rect.y-350).abs()>6 || (rect.width,rect.height)!=(416,250) {
            return Err("unexpected overlap geometry".into());
        }
        let raised=index==18;
        if (!raised && (moved_slot!=Some((1,rect.x,rect.y)) || raised_slot.is_some())) ||
            (raised && (raised_slot!=Some(1) || moved_slot.is_some())) {
            return Err("unexpected overlap/raise ticket".into());
        }
        let front=if raised {1} else {4};
        if layout.hit_test(800.0,550.0)!=Some(front) {
            return Err("overlap hit-test and stacking disagree".into());
        }
        let a=retained_gpu_probe_rgb(8).ok_or("missing A color")?;
        let c=retained_gpu_probe_rgb(9).ok_or("missing C color")?;
        let d=retained_gpu_probe_rgb(11).ok_or("missing D color")?;
        // Independently sample the exposed portions of BOTH overlapping
        // windows, the other survivor, and the contested actual GPU pixels.
        sample(renderer,700,400,a)?;
        sample(renderer,800,650,d)?;
        sample(renderer,300,500,c)?;
        sample(renderer,700,220,[27,33,58])?;
        sample(renderer,rect.x+rect.width+2,400,[65,171,216])?;
        sample(renderer,992,650,[65,171,216])?;
        sample(renderer,800,550,if raised {a} else {d})?;
        println!("MELANIA_NATIVE_RETAINED_GPU_STACK_PIXELS_VERIFIED={} front={front}",index+1);
        return Ok(());
    } else if raised_slot.is_some() {return Err("unexpected early GPU raise".into());}
    for (slot,frame) in expected_by_slot {
        let (w,h)=retained_gpu_probe_size(frame).ok_or("missing probe size")?;
        let expected=retained_gpu_probe_rgb(frame).ok_or("missing probe color")?;
        let (mut x,mut y,w,h)=retained_gpu_geometry(slot,w,h).ok_or("missing GPU probe geometry")?;
        if let Some((moved,px,py))=moved_slot {if moved==slot {x=px;y=py;}}
        if index>=15 && slot==1 {
            let rect=layout.geometry(slot).ok_or("restored GPU role lost geometry")?;
            x=rect.x;y=rect.y;
        }
        // Center AND far corner prove the actual retained extent.
        sample(renderer,x+w/2,y+h/2,expected)?;
        sample(renderer,x+w-3,y+h-3,expected)?;
        sample(renderer,x+w+2,y+h/2,[65,171,216])?;
        if frame==index {
            if let Some((old_w,old_h))=previous_size {
                if old_w>w+12 {
                    sample(renderer,x+old_w-2,y+h.min(old_h)/2,[27,33,58])?;
                    println!("MELANIA_NATIVE_RETAINED_GPU_SHRINK_CLEARED={}",index+1);
                } else if old_h>h+12 {
                    sample(renderer,x+w.min(old_w)/2,y+old_h-2,[27,33,58])?;
                    println!("MELANIA_NATIVE_RETAINED_GPU_SHRINK_CLEARED={}",index+1);
                }
                println!("MELANIA_NATIVE_RETAINED_GPU_RESIZE_VERIFIED={} {}x{}",index+1,w,h);
            }
        }
    }
    if (13..=14).contains(&index) {
        println!("MELANIA_NATIVE_RETAINED_GPU_MOVE_PIXELS_VERIFIED=1");
        println!("MELANIA_NATIVE_RETAINED_GPU_MOVE_FRAME_PIXELS_VERIFIED={}",index+1);
    }
    if index>=15 {
        println!("MELANIA_NATIVE_RETAINED_GPU_VISIBILITY_PIXELS_VERIFIED={} visible={}",index+1,index==16);
    }
    Ok(())
}

// GPU-owned immutable window snapshot. The client BO may be released
// after KMS ACK, but its pixels must remain available for later frames
// of other windows. Never map or download this retained texture.
fn retain_direct_window(
    renderer:&mut GlesRenderer,
    source:&mut smithay::backend::renderer::gles::GlesTexture,
    opaque:bool,
)->Result<smithay::backend::renderer::gles::GlesTexture,Box<dyn Error>> {
    use smithay::backend::renderer::{Offscreen,Texture};
    use smithay::backend::renderer::gles::GlesTexture;
    let (w,h)=(source.width() as i32,source.height() as i32);
    crate::window_placement::retained_gpu_geometry(1,w,h)
        .ok_or("retained GPU texture exceeds output geometry")?;
    let mut image:GlesTexture=renderer.create_buffer(Fourcc::Abgr8888,(w,h).into())?;
    {
        let mut target=renderer.bind(&mut image)?;
        let mut frame=renderer.render(&mut target,(w,h).into(),Transform::Normal)?;
        let rect=Rectangle::from_size((w,h).into());
        let src=Rectangle::from_size((f64::from(w),f64::from(h)).into());
        // Preserve premultiplied source alpha in GPU-owned storage.
        // Blend against the actual destination only when drawing the scene.
        frame.clear(Color32F::new(0.0,0.0,0.0,0.0),&[rect])?;
        let opaque_regions=if opaque {std::slice::from_ref(&rect)} else {&[]};
        if !opaque {
            frame.render_texture_from_to(source,src,rect,&[rect],opaque_regions,
                Transform::Flipped180,1.0,None,&[])?;
        }
        frame.finish()?.wait()?;
        if opaque {
            // XRGB has already had its alpha normalized. No opaque
            // texture shader in this snapshot stage, where llvmpipe crashed.
            blit_opaque_direct_window(renderer,source,(0,0))?;
            println!("MELANIA_NATIVE_RETAINED_GPU_OPAQUE_SNAPSHOT_BLIT=true");
        }
    }
    Ok(image)
}

fn qemu_gpu_scene_stream(
    mut stream:UnixStream, card_fd:OwnedFd,
    direct:Option<std::sync::mpsc::Receiver<crate::continuous_kms_stream::DirectGpuFrame>>,
)->Result<(),Box<dyn Error>> {
    const W:usize=1280;
    const H:usize=800;
    let direct_mode=direct.is_some();
    let multi_gpu=std::env::var("MELANIA_NATIVE_DIRECT_GPU_MULTIWINDOW")
        .as_deref()==Ok("YES_QEMU_VM");
    if multi_gpu && !direct_mode {return Err("retained GPU mode requires direct channel".into());}
    let total=if multi_gpu {13u64} else if direct_mode {12u64} else {24u64};
    let live_resize=multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("LIVE_RESIZE_QEMU_VM");
    let resize_gpu=live_resize || (multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("RESIZE_QEMU_VM"));
    let stack_gpu=resize_gpu || (multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("STACK_QEMU_VM"));
    let visibility_gpu=stack_gpu || (multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("VISIBILITY_QEMU_VM"));
    let live_drag=visibility_gpu || (multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("LIVE_QEMU_VM"));
    let move_gpu=live_drag || (multi_gpu && std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").as_deref()==Ok("YES_QEMU_VM"));
    let total=if live_resize {21} else if resize_gpu {20} else if stack_gpu {19} else if visibility_gpu {17} else if live_drag {15} else if move_gpu {14} else {total};
    // Explicit isolated acceptance mode, independent from the historical
    // finite source-lifetime diagnostic. Never enable for single texture
    // scanout, where the client import itself is used in later frames.
    let rolling_retire=std::env::var("MELANIA_NATIVE_GPU_ROLLING_RETIRE").as_deref()==Ok("YES_QEMU_VM");
    if std::env::var_os("MELANIA_NATIVE_GPU_ROLLING_RETIRE").is_some() &&
        (!rolling_retire || !multi_gpu || !direct_mode) {
        return Err("rolling GPU import retirement requires retained direct scene".into());
    }
    let persistent=std::env::var("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND")
        .as_deref()==Ok("YES_DISPOSABLE_QEMU");
    let persistent_direct=crate::native_frame_budget::persistent_direct_gpu_mode_matches(
        std::env::var("MELANIA_NATIVE_PERSISTENT_DIRECT_GPU").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_PERSISTENT_KMS").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_DIRECT_GPU_MULTIWINDOW").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_DIRECT_GPU_MOVE").ok().as_deref(),
        std::env::var("MELANIA_QEMU_CONTINUOUS_FRAMES").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_QEMU_FRAME_TARGET").ok().as_deref(),
        std::env::var("MELANIA_NATIVE_GPU_ROLLING_RETIRE").ok().as_deref(),
    );
    if (direct_mode && persistent && !persistent_direct) ||
       (std::env::var_os("MELANIA_NATIVE_PERSISTENT_DIRECT_GPU").is_some() &&
        (!persistent_direct || !direct_mode)) {
        return Err("direct client GPU proof cannot enter persistent mode without retained lifetime opt-in".into());
    }
    if persistent &&
       std::env::var("MELANIA_NATIVE_PERSISTENT_KMS").as_deref()
           !=Ok("YES_DISPOSABLE_QEMU") {
        return Err("persistent GPU Wayland must be owned by persistent QEMU libseat session".into());
    }
    // Idle is not a GPU failure. Only explicit socket shutdown after
    // SIGTERM/SIGINT ends the persistent renderer. Preserve the finite
    // test's 12-second stalled-transport timeout.
    stream.set_read_timeout(if persistent {
        None
    } else {
        Some(Duration::from_secs(12))
    })?;
    stream.set_write_timeout(Some(Duration::from_secs(7)))?;
    let card=DrmDeviceFd::new(DeviceFd::from(card_fd));
    let topology=crate::native_output_topology::inspect_stable_active_outputs(&card)?;
    let route=topology.single_active()?;
    let crtc=route.crtc;
    let connector=route.connector;
    let original_encoder=card.get_connector(connector,false)?
        .current_encoder().ok_or("Native GBM output missing its original connector encoder")?;
    // The stable DRM snapshot identified THIS encoder, not merely its
    // destination CRTC. A VT/hotplug takeover can swap the encoder while
    // keeping the same CRTC and framebuffer: do not adopt that new route.
    // Both the 24-frame and persistent GPU Wayland paths must check before
    // creating or reusing any GBM scanout buffers.
    if original_encoder!=route.original_encoder {
        return Err("GBM KMS original encoder changed since stable topology".into());
    }
    if card.get_encoder(original_encoder)?.crtc()!=Some(crtc) {
        return Err("Native GBM encoder route changed after topology acquisition".into());
    }
    let original=card.get_crtc(crtc)?;
    let mode=original.mode().ok_or("Wayland GPU CRTC has no active mode")?;
    if mode.size()!=(W as u16,H as u16) ||
       original.framebuffer()!=Some(route.original_fb) {
        return Err("Wayland GPU CRTC baseline framebuffer/mode changed".into());
    }
    crate::renderer::validate_gbm_output_dimensions(W as u32,H as u32,2)?;

    let gbm=GbmDevice::new(card.clone())?;
    let mut allocator=DmabufAllocator(GbmAllocator::new(
        gbm.clone(),GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    ));
    let mut a=allocator.create_buffer(W as u32,H as u32,
        Fourcc::Argb8888,&[Modifier::Linear])?;
    let mut b=allocator.create_buffer(W as u32,H as u32,
        Fourcc::Argb8888,&[Modifier::Linear])?;
    let fb_a=framebuffer_from_dmabuf(&card,&gbm,&a,true,true)?;
    let fb_b=framebuffer_from_dmabuf(&card,&gbm,&b,true,true)?;
    let handles=[*fb_a.as_ref(),*fb_b.as_ref()];
    let display=unsafe {EGLDisplay::new(gbm)}?;
    let context=EGLContext::new(&display)?;
    let mut renderer=if direct_mode {
        // The disposable VirtIO EGL path crashed during frame 2, while
        // single-frame pixel/CRTC verification passed. Isolate driver fence
        // export and delayed EGL image destruction from the experiment.
        // Smithay's finish falls back to actual glFinish without ExportFence;
        // this is still GPU rendering, never a synthetic completion.
        use smithay::backend::renderer::gles::Capability;
        let mut caps=unsafe {GlesRenderer::supported_capabilities(&context)}?;
        caps.retain(|cap| *cap!=Capability::ExportFence);
        let renderer=unsafe {GlesRenderer::with_capabilities(context,caps)}?;
        println!("MELANIA_NATIVE_DIRECT_GPU_FINISH_MODE=GL_FINISH");
        renderer
    } else {unsafe {GlesRenderer::new(context)}?};
    // The XRGB8888 wl_shm scene has no meaningful alpha channel.
    // Never interpret padding bytes as transparency on the target.
    if !renderer.mem_formats().any(|fmt|fmt==Fourcc::Xrgb8888) {
        return Err("QEMU GLES cannot import the compositor's XRGB8888 image".into());
    }
    let mut flip_ledger=crate::native_flip_ledger::NativeFlipLedger::default();
    let mut event_router=crate::native_drm_flip::DrmFlipEventRouter::default();
    event_router.bind_active_crtcs(&[crtc])?;
    let mut rollback=Restore {
        card:&card,original,connector,original_encoder,submitted:false,restored:false,ownership_lost:false,owned_fbs:handles,
    };
    println!("MELANIA_NATIVE_GPU_WAYLAND_KMS_READY=true");
    println!("MELANIA_NATIVE_GPU_WAYLAND_DRM_TOPOLOGY_OUTPUTS={}",
             topology.active.len());
    let mut frames_seen=0u64;
    // The finite direct proof owns at most twelve source allocations.
    // Keep imported image/FD handles alive across the next EGL draw instead
    // of allowing the client's destroy request to retire the driver's EGL
    // resources during the following frame. Never sample a released source.
    // Drop after kernel restoration; not a policy for unbounded sessions.
    let mut normalized_gpu_sources=Vec::<smithay::backend::renderer::gles::GlesTexture>::new();
    let mut direct_import_lifetime=Vec::<(
        smithay::backend::allocator::dmabuf::Dmabuf,
        smithay::backend::renderer::gles::GlesTexture,
    )>::new();
    let mut rolling_imports=crate::continuous_kms_stream::GpuImportRetirement::<(
        smithay::backend::allocator::dmabuf::Dmabuf,
        smithay::backend::renderer::gles::GlesTexture,
        Option<smithay::backend::renderer::gles::GlesTexture>,
    )>::default();
    // Keeping only the retired backing did not prevent fifth-import
    // EGLImage failure in the separate Smithay importer. Hold the full
    // retired source across one additional verified draw/DRM receipt.
    // At most three source bundles, including normalized textures.
    let mut rolling_deferred:Option<(
        smithay::backend::allocator::dmabuf::Dmabuf,
        smithay::backend::renderer::gles::GlesTexture,
        Option<smithay::backend::renderer::gles::GlesTexture>,
    )>=None;
    let mut rolling_backing_peak=0usize;
    let mut rolling_retired=0usize;
    let mut rolling_peak=0usize;
    let mut rolling_staged=0usize;
    let attempt=(||->Result<(),Box<dyn Error>> {
        let mut prior_identity:Option<crate::native_frame_order::FrameIdentity>=None;
        // Retain two fixed-size CPU staging buffers for the entire GPU
        // presenter lifetime. Neither a client commit nor a KMS flip may
        // allocate another 4 MiB scene Vec. The previous buffer is marked
        // confirmed ONLY after the kernel scanout check and successful ACK.
        let mut pixels=vec![0u8;W*H*4];
        let mut prior_pixels=vec![0u8;W*H*4];
        let mut have_prior_pixels=false;
        // At most the registry's bounded number of independently retained
        // texture allocations. IDs do not shift when another surface commits.
        let mut retained=std::collections::BTreeMap::<u64,smithay::backend::renderer::gles::GlesTexture>::new();
        let mut retained_layout=crate::retained_gpu_layout::RetainedGpuLayout::default();
        loop {
            if !persistent && frames_seen>=total {break;}
            let index=frames_seen;
            // Never wrap the local presentation ordinal used by retirement.
            if index==u64::MAX {return Err("GPU presentation ordinal exhausted".into());}
            let mut header=[0u8;32];
            // The SAME real DRM ownership checker now protects the idle
            // socket, partial header and incomplete pixel buffer. A client
            // cannot hold a stolen GBM CRTC until the 30s scene deadline.
            let expected_read_fb=if index==0 {route.original_fb}
                else {handles[((index-1)%2) as usize]};
            let mut verify_read_drm=||{
                let owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                    &card,crtc,connector,original_encoder,expected_read_fb,
                    (W as u16,H as u16),
                mode,original.position());
                match owned {
                    Ok(true)=>Ok(()),
                    Ok(false) | Err(_) => {
                        event_router.revoke();
                        rollback.ownership_lost=true;
                        Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "Persistent GBM output lost original DRM scanout during frame reception",
                        ))
                    }
                }
            };
            let pending_read=if persistent {
                let Some(read)=crate::native_persistent_frame_receive::
                    PersistentFrameRead::begin_with_idle_guard(
                        &mut stream,&mut header,
                        crate::native_persistent_frame_receive::MAX_PARTIAL_FRAME_TIME,
                        &mut verify_read_drm,
                    )? else {break;};
                Some(read)
            } else {
                stream.read_exact(&mut header)?;
                None
            };
            let direct_frame=direct_mode && (index<2 || multi_gpu);
            let expected_marker=if direct_frame {b"MLG1"} else {b"MLF1"};
            if &header[..4]!=expected_marker {
                return Err("GPU Wayland frame header has wrong protocol version".into());
            }
            let epoch=u64::from_le_bytes(header[4..12].try_into()?);
            let sequence=u64::from_le_bytes(header[12..20].try_into()?);
            let generation=u64::from_le_bytes(header[20..28].try_into()?);
            let length=u32::from_le_bytes(header[28..32].try_into()?) as usize;
            if length!=(if direct_frame {0} else {W*H*4}) || sequence==0 {
                return Err("GPU Wayland scene has invalid frame buffer dimensions".into());
            }
            let identity=crate::native_frame_order::FrameIdentity::new(
                epoch,sequence,generation);
            if !identity.accepted_after(prior_identity,persistent) {
                return Err("GPU Wayland compositor sent stale or reordered frame".into());
            }
            let mut removed_slot=None;
            let mut moved_slot=None;
            let mut visibility_slot=None;
            let mut raised_slot=None;
            let mut resize_target=None;
            let client_dmabuf=if direct_frame {
                let (planned,slot,buffer)=direct.as_ref().ok_or("direct GPU channel missing")?
                    .recv_timeout(Duration::from_secs(2))?;
                if planned.ticket.epoch!=epoch || planned.ticket.sequence!=sequence ||
                   planned.generation!=generation {
                    return Err("direct GPU buffer ticket mismatch".into());
                }
                if crate::window_placement::retained_gpu_position(slot).is_none() ||
                    (!multi_gpu && slot!=1) {
                    return Err("unexpected direct GPU surface slot".into());
                }
                use crate::continuous_kms_stream::DirectGpuUpdate;
                match buffer {
                    DirectGpuUpdate::Buffer(buffer)=>Some((slot,buffer)),
                    DirectGpuUpdate::Resize{buffer,rect} if persistent_direct || (resize_gpu && (index==19 || (live_resize && index==20)))=>{
                        if !retained.contains_key(&slot) || retained_layout.is_visible(slot)!=Some(true) {
                            return Err("GPU resize references missing or hidden surface".into());
                        }
                        resize_target=Some(rect);
                        Some((slot,buffer))
                    },
                    DirectGpuUpdate::Remove if multi_gpu=>{
                        if retained.remove(&slot).is_none() || !retained_layout.remove(slot) {
                            return Err("GPU removal references an absent surface".into());
                        }
                        removed_slot=Some(slot);
                        None
                    }
                    DirectGpuUpdate::Move{x,y} if move_gpu || persistent_direct=>{
                        if !retained.contains_key(&slot) || !retained_layout.move_to(slot,x,y)? {
                            return Err("GPU motion references missing or unchanged surface".into());
                        }
                        let rect=retained_layout.geometry(slot).ok_or("missing moved GPU geometry")?;
                        moved_slot=Some((slot,rect.x,rect.y));
                        None
                    }
                    DirectGpuUpdate::Visibility{visible} if visibility_gpu || persistent_direct=>{
                        if !retained.contains_key(&slot) || !retained_layout.set_visible(slot,visible)? {
                            return Err("GPU visibility references missing or unchanged surface".into());
                        }
                        visibility_slot=Some((slot,visible));
                        None
                    }
                    DirectGpuUpdate::Raise if stack_gpu || persistent_direct=>{
                        if !retained.contains_key(&slot) || !retained_layout.raise(slot)? {
                            return Err("GPU raise references missing, hidden or unchanged surface".into());
                        }
                        raised_slot=Some(slot);
                        None
                    }
                    _=>return Err("unsupported direct GPU scene operation".into()),
                }
            } else {
                if let Some(read)=pending_read {
                    read.read_payload_guarded(&mut stream,&mut pixels,&mut verify_read_drm)?;
                } else {
                    stream.read_exact(&mut pixels)?;
                }
                None
            };
            if !direct_frame && have_prior_pixels && prior_pixels==pixels && !persistent {
                // Preserve the finite QEMU pixel-change acceptance. In a
                // persistent desktop, however, a real Wayland client may
                // commit the same image again (e.g. frame callback request,
                // unmodified canvas or expose). It must not kill the session.
                // This still submits a real KMS frame: the client receives
                // no ACK until the kernel pageflip/FB_ID route is verified.
                return Err("GPU Wayland finite probe did not change pixels".into());
            }

            // This is the native compositor's real, owned Wayland wl_shm
            // composite, not a test card or synthetic EGL clear color.
            // Import the genuine client DMA-BUF directly into the scanout
            // GPU context, revalidating its producer fence at sampling.
            // SHM-only frames retain the historical memory upload.
            if direct_frame {println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} import",index+1);}
            let mut texture=if let Some((slot,buffer))=client_dmabuf.as_ref() {
                use smithay::backend::allocator::Buffer;
                let size=buffer.size();
                let valid_size=if multi_gpu {
                    crate::window_placement::retained_gpu_geometry(*slot,size.w,size.h).is_some()
                } else {size.w==360 && size.h==224};
                if !valid_size ||
                   !matches!(buffer.format().code,Fourcc::Argb8888|Fourcc::Xrgb8888) ||
                   (!multi_gpu && buffer.format().code!=(if index==0 {Fourcc::Argb8888} else {Fourcc::Xrgb8888})) {
                    return Err("direct GPU client size/format mismatch".into());
                }
                Some(crate::native_client_dmabuf::import_client_dmabuf(&mut renderer,buffer)?)
            } else if removed_slot.is_some() || moved_slot.is_some() || visibility_slot.is_some() || raised_slot.is_some() {
                None
            } else {
                Some(renderer.import_memory(
                    &pixels,Fourcc::Xrgb8888,(W as i32,H as i32).into(),false,
                )?)
            };
            let mut opaque_snapshot=if direct_frame && client_dmabuf.as_ref().is_some_and(|(_,buffer)|{
                use smithay::backend::allocator::Buffer;
                buffer.format().code==Fourcc::Xrgb8888
            }) {
                println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} opaque_blit",index+1);
                let image=normalize_opaque_direct_source(&mut renderer,texture.as_mut().ok_or("missing opaque texture")?)?;
                println!("MELANIA_NATIVE_DIRECT_GPU_XRGB_NORMALIZED=true");
                Some(image)
            } else {None};
            if multi_gpu && removed_slot.is_none() && moved_slot.is_none() && visibility_slot.is_none() && raised_slot.is_none() {
                use smithay::backend::allocator::Buffer;
                let (slot,buffer)=client_dmabuf.as_ref().ok_or("missing retained GPU source")?;
                let opaque=buffer.format().code==Fourcc::Xrgb8888;
                // Replace only this real surface's image, regardless of
                // commit order, source Fourcc or presentation sequence.
                let source=opaque_snapshot.as_mut().or(texture.as_mut()).ok_or("missing retained source texture")?;
                let image=retain_direct_window(&mut renderer,source,opaque)?;
                let size=buffer.size();
                if let Some(rect)=resize_target {
                    retained_layout.commit_resize(*slot,rect,size.w,size.h)?;
                } else {
                    retained_layout.update_buffer(*slot,size.w,size.h)?;
                }
                retained.insert(*slot,image);
            }
            let sampled_texture=opaque_snapshot.as_ref().or(texture.as_ref());
            if direct_frame {println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} bind",index+1);}
            let target=if index%2==0 {&mut a} else {&mut b};
            let mut bound=renderer.bind(target)?;
            let mut frame=renderer.render(&mut bound,
                (W as i32,H as i32).into(),Transform::Normal)?;
            let output_rect=Rectangle::from_size((W as i32,H as i32).into());
            if direct_frame {println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} draw",index+1);}
            if multi_gpu {
                let bg=Color32F::new(27.0/255.0,33.0/255.0,58.0/255.0,1.0);
                frame.clear(bg,&[output_rect])?;
                // Geometry and stacking belong to the compositor, not to
                // the latest client buffer. Draw back-to-front from the
                // same scene model that can later receive pointer moves.
                if retained_layout.bottom_to_top().count()!=retained.len() {
                    return Err("GPU texture and layout ownership diverged".into());
                }
                for (slot,rect) in retained_layout.bottom_to_top() {
                    use smithay::backend::renderer::Texture;
                    let image=retained.get(&slot).ok_or("missing retained GPU window texture")?;
                    let (x,y,w,h)=(rect.x,rect.y,rect.width,rect.height);
                    if (image.width() as i32,image.height() as i32)!=(w,h) {
                        return Err("retained GPU geometry/texture dimensions diverged".into());
                    }
                    if retained_layout.is_visible(slot)!=Some(true) {continue;}
                    let border=[
                        Rectangle::from_loc_and_size((x-6,y-6),(w+12,6)),
                        Rectangle::from_loc_and_size((x-6,y+h),(w+12,6)),
                        Rectangle::from_loc_and_size((x-6,y),(6,h)),
                        Rectangle::from_loc_and_size((x+w,y),(6,h)),
                    ];
                    let dest=Rectangle::from_loc_and_size((x,y),(w,h));
                    let local=Rectangle::from_size((w,h).into());
                    let src=Rectangle::from_size((f64::from(w),f64::from(h)).into());
                    frame.clear(Color32F::new(65.0/255.0,171.0/255.0,216.0/255.0,1.0),&border)?;
                    // Transparent client pixels reveal the composed scene,
                    // not a flattened copy of the original desktop color.
                    frame.render_texture_from_to(image,src,dest,&[local],&[],
                        Transform::Normal,1.0,None,&[])?;
                }
            } else if direct_frame {
                // Entire single-window output (background, decorations and
                // client pixels) is rendered into the actual GBM scanout BO.
                let border=Rectangle::from_loc_and_size((154,114),(372,236));
                let dest=Rectangle::from_loc_and_size((160,120),(360,224));
                let src=Rectangle::from_size((360.0,224.0).into());
                let bg=Color32F::new(27.0/255.0,33.0/255.0,58.0/255.0,1.0);
                frame.clear(bg,&[output_rect])?;
                frame.clear(Color32F::new(65.0/255.0,171.0/255.0,216.0/255.0,1.0),&[border])?;
                frame.clear(bg,&[dest])?;
                // Damage and opaque regions are local to the destination.
                let local=Rectangle::from_size((360,224).into());
                let opaque=if index==1 {std::slice::from_ref(&local)} else {&[]};
                // Match the existing compositor orientation: its old
                // CPU staging path flipped the bottom-up GLES readback
                // before uploading the top-origin scene. Do that transform
                // in the GPU shader, rather than copying rows on the CPU.
                if index==0 {
                    frame.render_texture_from_to(sampled_texture.ok_or("missing direct sample texture")?,src,dest,&[local],opaque,
                        Transform::Flipped180,1.0,None,&[])?;
                }
                // Opaque XRGB is copied by framebuffer blit after clear;
                // do not run the llvmpipe texture shader that crashed.
            } else {
                let src=Rectangle::from_size((W as f64,H as f64).into());
                frame.clear(Color32F::new(0.0,0.0,0.0,1.0),&[output_rect])?;
                frame.render_texture_from_to(
                    texture.as_ref().ok_or("missing CPU upload texture")?,src,output_rect,&[output_rect],&[],
                    Transform::Normal,1.0,None,&[],
                )?;
            }
            if direct_frame {
                // Distinguish a driver flush crash from Smithay's deferred
                // deletion during finish. This does not replace completion.
                println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} driver_finish",index+1);
                frame.with_context(|gl|unsafe {gl.Finish()})?;
                println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} driver_finished",index+1);
                println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} finish",index+1);
            }
            let completion=frame.finish()?;

            if direct_frame {println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} wait",index+1);}
            completion.wait()?;
            if !multi_gpu {
                if let Some(image)=opaque_snapshot.as_mut() {
                    println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} output_blit",index+1);
                    blit_opaque_direct_window(&mut renderer,image,(160,120))?;
                    println!("MELANIA_NATIVE_DIRECT_GPU_XRGB_OUTPUT_BLIT=true");
                }
            }
            if direct_frame {println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} verify",index+1);}
            if multi_gpu && !persistent_direct {
                // All existing deterministic fixture pixel checks stay mandatory.
                verify_retained_gpu_scene(&mut renderer,&bound,index,removed_slot,moved_slot,visibility_slot,raised_slot,&retained_layout)?;
            } else if persistent_direct {
                // General client scenes have no expected fixture colors.
                // Validate the complete retained scene (including hidden
                // windows) and still require actual GPU completion + DRM ACK.
                if retained_layout.bottom_to_top().count()!=retained.len() ||
                   retained_layout.bottom_to_top().any(|(slot,rect)|{
                       use smithay::backend::renderer::Texture;
                       !retained.get(&slot).is_some_and(|texture|
                           (texture.width() as i32,texture.height() as i32)==(rect.width,rect.height))
                   }) {
                    return Err("persistent GPU retained scene geometry mismatch".into());
                }
            } else if direct_frame {
                verify_direct_client_gpu_pixels(&mut renderer,&bound,index)?;
            }
            drop(bound);
            if direct_frame && removed_slot.is_none() && moved_slot.is_none() && visibility_slot.is_none() && raised_slot.is_none() {
                let (_,buffer)=client_dmabuf.ok_or("direct client import disappeared")?;
                let texture=texture.ok_or("direct client texture disappeared")?;
                if rolling_retire {
                    rolling_imports.stage(index+1,(buffer,texture,opaque_snapshot))?;
                    rolling_staged+=1;
                    rolling_peak=rolling_peak.max(rolling_imports.len());
                    rolling_backing_peak=rolling_backing_peak.max(rolling_imports.len()+usize::from(rolling_deferred.is_some()));
                    if rolling_backing_peak>3 {return Err("GPU source backing budget exceeded".into());}
                } else {
                    if direct_import_lifetime.len()>=12 &&
                        (!resize_gpu || direct_import_lifetime.len()>=if live_resize {14} else {13}) {
                        return Err("finite direct GPU import lifetime budget exceeded".into());
                    }
                    direct_import_lifetime.push((buffer,texture));
                    if let Some(image)=opaque_snapshot {normalized_gpu_sources.push(image);}
                }
                println!("MELANIA_NATIVE_DIRECT_GPU_STAGE={} retained",index+1);
            }

            // A previously confirmed scanout may have been taken over
            // between frames. Never render/submit into a lost DRM route.
            let previous_fb=if index==0 {route.original_fb}
                else {handles[(index-1) as usize%2]};
            let owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                &card,crtc,connector,original_encoder,previous_fb,(W as u16,H as u16),
            mode,original.position()).map_err(|error| {
                event_router.revoke();
                rollback.ownership_lost=true;
                error
            })?;
            if !owned {
                event_router.revoke();
                rollback.ownership_lost=true;
                return Err("GPU Wayland lost DRM connector/CRTC/mode/FB ownership before submission".into());
            }
            let framebuffer=handles[index as usize%2];
            if index==0 {
                rollback.submitted=true;
                card.set_crtc(crtc,Some(framebuffer),original.position(),
                              &[connector],Some(mode))?;
            } else {
                flip_ledger.begin_flip()
                    .map_err(|err|format!("Wayland GPU duplicate flip: {err:?}"))?;
                let deadline=Instant::now()+Duration::from_secs(3);
                loop {
                    // The frame was rendered before this ioctl, but EAGAIN
                    // can sleep through a VT pause or foreign modeset. Check
                    // the actual kernel route before each subsequent retry.
                    let retry_owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                        &card,crtc,connector,original_encoder,previous_fb,
                        (W as u16,H as u16),
                    mode,original.position()).map_err(|error|{
                        event_router.revoke();
                        rollback.ownership_lost=true;
                        error
                    })?;
                    if !retry_owned {
                        event_router.revoke();
                        rollback.ownership_lost=true;
                        return Err("GPU Wayland page-flip retry lost original DRM ownership".into());
                    }
                    match card.page_flip(crtc,framebuffer,PageFlipFlags::EVENT,None) {
                        Ok(())=>{
                            event_router.arm_flip(crtc)?;
                            break;
                        },
                        Err(err) if err.raw_os_error()==Some(11) &&
                            Instant::now()<deadline => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(err)=>return Err(err.into()),
                    }
                }
                let vblank=crate::native_drm_flip::wait_for_kms_page_flip_routed(
                    &card,crtc,Duration::from_secs(3),&mut event_router,
                )?;
                flip_ledger.complete_vm_flip(vblank)
                    .map_err(|err|format!("Wayland GPU VBlank mismatch: {err:?}"))?;
            }
            // A kernel page flip event alone is NOT evidence that the desired
            // framebuffer was routed to this physical CRTC/connector.
            wait_for_scanout(&card,crtc,connector,framebuffer)?;
            let ack_owned=crate::native_kms_scanout::verify_original_encoder_frame_at_baseline(
                &card,crtc,connector,original_encoder,framebuffer,(W as u16,H as u16),
            mode,original.position()).map_err(|error|{
                event_router.revoke();
                rollback.ownership_lost=true;
                error
            })?;
            if !ack_owned {
                event_router.revoke();
                rollback.ownership_lost=true;
                return Err("GPU Wayland original encoder changed before client ACK".into());
            }
            println!("MELANIA_NATIVE_GPU_WAYLAND_ORIGINAL_ENCODER_FRAME_ACK={}",index+1);
            println!("MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE={}",index+1);
            if index>0 {
                println!("MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER={}",
                         flip_ledger.completed_count());
            }
            if rolling_retire {
                // At this point GLES completion and the real kernel route,
                // connector, mode and framebuffer verification succeeded.
                // Never evict an import on a failed submission/flip.
                // Preserve the entire previous bundle, not only its FD.
                // Drop normalized/imported textures before the backing,
                // never purge the EGL cache in the live rendering loop.
                let mut next_deferred=None;
                let retired=rolling_imports.confirm_with(index+1,|source|{
                    debug_assert!(next_deferred.is_none());
                    next_deferred=Some(source);
                })?;
                if let Some((buffer,texture,normalized))=rolling_deferred.take() {
                    drop(normalized);
                    drop(texture);
                    drop(buffer);
                }
                rolling_deferred=next_deferred;
                rolling_retired+=retired;
                if retired>0 {
                    println!("MELANIA_NATIVE_GPU_ROLLING_IMPORTS_RETIRED={} frame={}",retired,index+1);
                }
            }
            if !persistent && index+1==total {
                // Finite mode must restore before its last client ACK.
                rollback.restore()?;
            }
            let mut ack=[0u8;29];
            ack[..4].copy_from_slice(b"MLA1");
            ack[4..12].copy_from_slice(&epoch.to_le_bytes());
            ack[12..20].copy_from_slice(&sequence.to_le_bytes());
            ack[20..28].copy_from_slice(&generation.to_le_bytes());
            ack[28]=0;
            stream.write_all(&ack)?;
            stream.flush()?;
            println!("MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK={}",index+1);
            if direct_frame {
                println!("MELANIA_NATIVE_DIRECT_CLIENT_GBM_SCANOUT_ACK={}",index+1);
                if multi_gpu {
                    println!("MELANIA_NATIVE_RETAINED_GPU_SCENE_ACK={} windows={}",
                        index+1,retained.len());
                }
            }
            frames_seen=frames_seen.checked_add(1)
                .ok_or("GPU Wayland compositor frame counter exhausted")?;
            prior_identity=Some(identity);
            if !direct_frame {
                std::mem::swap(&mut pixels,&mut prior_pixels);
                have_prior_pixels=true;
            }
        }
        Ok(())
    })();
    let restored=rollback.restore();
    attempt?;
    restored?;
    if direct_mode {
        let mut final_backings=Vec::new();
        // All draws returned actual GPU completion and the kernel has
        // restored its original CRTC. The old client images are never used
        // for future rendering; retire them now under the owning context.
        if rolling_retire {
            if rolling_peak>2 || rolling_imports.len()>1 ||
                rolling_retired+rolling_imports.len()!=rolling_staged {
                return Err("GPU rolling import retirement ledger mismatch".into());
            }
            println!("MELANIA_NATIVE_GPU_ROLLING_IMPORTS_PEAK={rolling_peak}");
            println!("MELANIA_NATIVE_GPU_ROLLING_IMPORTS_STAGED={rolling_staged}");
            println!("MELANIA_NATIVE_GPU_ROLLING_IMPORTS_RETIRED_BEFORE_RESTORE={rolling_retired}");
            println!("MELANIA_NATIVE_GPU_ROLLING_BACKINGS_PEAK={rolling_backing_peak}");
            println!("MELANIA_NATIVE_GPU_ROLLING_SOURCE_BUNDLES_PEAK={rolling_backing_peak}");
            rolling_imports.clear_after_restore_with(|(buffer,texture,normalized)|{
                drop(normalized);
                drop(texture);
                final_backings.push(buffer);
            });
            if let Some((buffer,texture,normalized))=rolling_deferred.take() {
                drop(normalized);
                drop(texture);
                final_backings.push(buffer);
            }
            if final_backings.len()>2 {return Err("GPU final backing budget exceeded".into());}
        }
        normalized_gpu_sources.clear();
        direct_import_lifetime.clear();
        renderer.cleanup_texture_cache()?;
        drop(final_backings);
        if rolling_retire {println!("MELANIA_NATIVE_GPU_ROLLING_IMPORTS_DRAINED=true");}
        println!("MELANIA_NATIVE_DIRECT_GPU_RETIRED_AFTER_RESTORE=true");
    }
    println!("MELANIA_NATIVE_GPU_WAYLAND_GBM_BUFFERS_REUSED=2");
    if persistent {
        if persistent_direct {
            println!("MELANIA_NATIVE_PERSISTENT_DIRECT_GPU_STOPPED_FRAMES={frames_seen}");
        }
        println!("MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_STOPPED_FRAMES={frames_seen}");
        println!("MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_ROLLBACK_PASS=true");
    } else if direct_mode {
        println!("MELANIA_NATIVE_DIRECT_CLIENT_GBM_12_KMS_FRAMES_PASS=true");
        if multi_gpu {
            println!("MELANIA_NATIVE_RETAINED_GPU_FOUR_WINDOW_12_FRAMES_PASS=true");
            println!("MELANIA_NATIVE_RETAINED_GPU_CLOSE_13_FRAMES_PASS=true");
            if stack_gpu {println!("MELANIA_NATIVE_RETAINED_GPU_STACK_19_FRAMES_PASS=true");}
            if resize_gpu {println!("MELANIA_NATIVE_GPU_XDG_RESIZE_20_FRAMES_PASS=true");}
            if live_resize {println!("MELANIA_NATIVE_GPU_XDG_LIVE_RESIZE_21_FRAMES_PASS=true");}
            if visibility_gpu {println!("MELANIA_NATIVE_RETAINED_GPU_VISIBILITY_17_FRAMES_PASS=true");}
            if live_drag {println!("MELANIA_NATIVE_RETAINED_GPU_LIVE_DRAG_15_FRAMES_PASS=true");}
            else if move_gpu {println!("MELANIA_NATIVE_RETAINED_GPU_REAL_DRAG_14_FRAMES_PASS=true");}
        }
    } else {
        println!("MELANIA_NATIVE_GPU_WAYLAND_24_KMS_FRAMES_PASS=true");
    }
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}
