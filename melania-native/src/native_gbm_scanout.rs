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
            let owned=crate::native_kms_scanout::verify_presenter_ownership(
                &card,crtc,connector,prior_fb,(w,h),
            ).map_err(|error| {
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
    mut stream:UnixStream, card_fd:OwnedFd,
)->Result<(),Box<dyn Error>> {
    const W:usize=1280;
    const H:usize=800;
    const TOTAL:u64=24;
    let persistent=std::env::var("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND")
        .as_deref()==Ok("YES_DISPOSABLE_QEMU");
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
    let mut renderer=unsafe {GlesRenderer::new(context)}?;
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
    let attempt=(||->Result<(),Box<dyn Error>> {
        let mut prior_identity:Option<crate::native_frame_order::FrameIdentity>=None;
        // Retain two fixed-size CPU staging buffers for the entire GPU
        // presenter lifetime. Neither a client commit nor a KMS flip may
        // allocate another 4 MiB scene Vec. The previous buffer is marked
        // confirmed ONLY after the kernel scanout check and successful ACK.
        let mut pixels=vec![0u8;W*H*4];
        let mut prior_pixels=vec![0u8;W*H*4];
        let mut have_prior_pixels=false;
        loop {
            if !persistent && frames_seen>=TOTAL {break;}
            let index=frames_seen;
            let mut header=[0u8;32];
            if persistent {
                // EOF is valid only at a complete frame boundary, never
                // after a partially transferred MLF1 header or image.
                if stream.read(&mut header[..1])?==0 {break;}
                stream.read_exact(&mut header[1..])?;
            } else {
                stream.read_exact(&mut header)?;
            }
            if &header[..4]!=b"MLF1" {
                return Err("GPU Wayland frame header has wrong protocol version".into());
            }
            let epoch=u64::from_le_bytes(header[4..12].try_into()?);
            let sequence=u64::from_le_bytes(header[12..20].try_into()?);
            let generation=u64::from_le_bytes(header[20..28].try_into()?);
            let length=u32::from_le_bytes(header[28..32].try_into()?) as usize;
            if length!=W*H*4 || sequence==0 {
                return Err("GPU Wayland scene has invalid frame buffer dimensions".into());
            }
            let identity=crate::native_frame_order::FrameIdentity::new(
                epoch,sequence,generation);
            if !identity.accepted_after(prior_identity,persistent) {
                return Err("GPU Wayland compositor sent stale or reordered frame".into());
            }
            stream.read_exact(&mut pixels)?;
            if have_prior_pixels && prior_pixels==pixels && !persistent {
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
            let texture=renderer.import_memory(
                &pixels,Fourcc::Xrgb8888,(W as i32,H as i32).into(),false,
            )?;
            let target=if index%2==0 {&mut a} else {&mut b};
            let mut bound=renderer.bind(target)?;
            let mut frame=renderer.render(&mut bound,
                (W as i32,H as i32).into(),Transform::Normal)?;
            let output_rect=Rectangle::from_size((W as i32,H as i32).into());
            let src=Rectangle::from_size((W as f64,H as f64).into());
            frame.clear(Color32F::new(0.0,0.0,0.0,1.0),&[output_rect])?;
            // GlesFrame's concrete overload expects custom shader args
            // even when the default texture shader is selected.
            frame.render_texture_from_to(
                &texture,src,output_rect,&[output_rect],&[],
                Transform::Normal,1.0,None,&[],
            )?;
            frame.finish()?.wait()?;
            drop(bound);

            // A previously confirmed scanout may have been taken over
            // between frames. Never render/submit into a lost DRM route.
            let previous_fb=if index==0 {route.original_fb}
                else {handles[(index-1) as usize%2]};
            let owned=crate::native_kms_scanout::verify_presenter_ownership(
                &card,crtc,connector,previous_fb,(W as u16,H as u16),
            ).map_err(|error| {
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
            println!("MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE={}",index+1);
            if index>0 {
                println!("MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER={}",
                         flip_ledger.completed_count());
            }
            if !persistent && index+1==TOTAL {
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
            frames_seen=frames_seen.checked_add(1)
                .ok_or("GPU Wayland compositor frame counter exhausted")?;
            prior_identity=Some(identity);
            std::mem::swap(&mut pixels,&mut prior_pixels);
            have_prior_pixels=true;
        }
        Ok(())
    })();
    let restored=rollback.restore();
    attempt?;
    restored?;
    println!("MELANIA_NATIVE_GPU_WAYLAND_GBM_BUFFERS_REUSED=2");
    if persistent {
        println!("MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_STOPPED_FRAMES={frames_seen}");
        println!("MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_ROLLBACK_PASS=true");
    } else {
        println!("MELANIA_NATIVE_GPU_WAYLAND_24_KMS_FRAMES_PASS=true");
    }
    println!("NATIVE_SESSION_READY=false");
    Ok(())
}
