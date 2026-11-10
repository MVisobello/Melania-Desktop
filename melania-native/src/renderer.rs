//! Native GPU capability probe: real GBM + EGL + OpenGL ES initialization.
//!
//! This never creates a KMS surface, framebuffer, DRM mode, display output,
//! compositor, or client. It can only be called from the existing guarded,
//! short-lived --experimental-drm path on a dedicated TTY/VM.
//!
//! Initialization belongs to a single GPU context lifetime. The caller
//! retains the libseat-owned DRM fd; everything here is dropped on return.
use smithay::backend::{
    allocator::gbm::GbmDevice,
    drm::DrmDeviceFd,
    egl::{EGLContext, EGLDisplay},
    renderer::gles::GlesRenderer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuRenderCapabilities {
    pub gbm_backend: String,
    pub dmabuf_render_formats: usize,
}

/// Actual EGL + GLES initialization against a libseat-owned DRM descriptor.
///
/// In particular, obtaining an OpenGL renderer does NOT imply any primary
/// plane, compatible DRM format, scanout, or VT pause/resume support. Those
/// must be validated before an independent native Wayland session exists.
pub fn inspect_gbm_egl(fd: DrmDeviceFd) -> Result<GpuRenderCapabilities, String> {
    let gbm=GbmDevice::new(fd).map_err(|error|format!("GBM device unavailable: {error}"))?;
    let backend=gbm.backend_name().to_string();
    // Safety: libseat owns the valid DRM descriptor; the GBM object is alive
    // throughout EGL creation/use and all EGL objects are dropped together
    // before this function returns. No raw host pointers are accepted.
    let display=unsafe {EGLDisplay::new(gbm)}
        .map_err(|error|format!("EGL display unavailable: {error}"))?;
    let context=EGLContext::new(&display)
        .map_err(|error|format!("EGL context unavailable: {error}"))?;
    // Safety: Smithay establishes a valid EGL context and owns GL resources.
    // There is no framebuffer binding, presentation or KMS state mutation.
    let _renderer=unsafe {GlesRenderer::new(context)}
        .map_err(|error|format!("GLES renderer unavailable: {error}"))?;
    Ok(GpuRenderCapabilities {
        gbm_backend:backend,
        dmabuf_render_formats:display.dmabuf_render_formats().iter().count(),
    })
}

/// Draw a tiny 64x64 GLES test frame into a GBM DMA-BUF, entirely offscreen.
///
/// Unlike stage 5 capability discovery, this allocates one temporary render
/// buffer and exercises GPU drawing/synchronization. It still cannot perform
/// KMS scanout, modeset, page flip, or write a device framebuffer.
/// Must only be called from the explicitly guarded experimental VM/TTY path.
/// Enforce a conservative allocation budget before a dedicated-VT GPU test.
/// This is a capacity guard, NOT an atomic scanout-format compatibility proof.
pub fn validate_gbm_output_dimensions(
    width:u32,height:u32,buffer_count:u32,
) -> Result<usize,String> {
    if width==0 || height==0 || width>4096 || height>4096 ||
       !(1..=3).contains(&buffer_count) {
        return Err("invalid native GPU output dimensions or buffer count".into());
    }
    let bytes=(width as usize).checked_mul(height as usize)
        .and_then(|n|n.checked_mul(4))
        .and_then(|n|n.checked_mul(buffer_count as usize))
        .ok_or_else(||"native GPU output allocation overflow".to_string())?;
    if bytes>96*1024*1024 {
        return Err("native GPU output exceeds the 96 MiB probe budget".into());
    }
    Ok(bytes)
}

/// Legacy 64x64 GPU probe retained for existing VM/TTY tests.
pub fn smoke_render_gbm_offscreen(fd: DrmDeviceFd) -> Result<(),String> {
    smoke_render_gbm_output_buffers(fd,64,64,1)
}

/// Synchronize successive GPU-rendered DMA-BUFs at the actual advertised
/// monitor resolution. Nothing is presented to a DRM plane and no mode is
/// changed: the dedicated-VT caller keeps ownership of the libseat FD.
///
/// A failed allocation, GLES render or sync is a hard failure, not a
/// fabricated KMS-ready signal. The future native backend must additionally
/// import compatible buffers, preserve the previous scanout and handle VT.
pub fn smoke_render_gbm_output_buffers(
    fd: DrmDeviceFd,width:u32,height:u32,frames:u32,
) -> Result<(),String> {
    use smithay::backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags},
            dmabuf::DmabufAllocator,
            Allocator, Fourcc, Modifier,
        },
        renderer::{Bind, Color32F, Frame, Renderer},
    };
    use smithay::utils::{Rectangle,Transform};

    validate_gbm_output_dimensions(width,height,frames)?;
    let gbm=GbmDevice::new(fd).map_err(|e|format!("GBM offscreen unavailable: {e}"))?;
    let mut allocator=DmabufAllocator(GbmAllocator::new(gbm.clone(),GbmBufferFlags::RENDERING));

    // The valid GBM device and libseat FD remain live until every
    // framebuffer and explicit GPU fence has been released.
    let display=unsafe {EGLDisplay::new(gbm)}
        .map_err(|e|format!("EGL display unavailable: {e}"))?;
    let context=EGLContext::new(&display)
        .map_err(|e|format!("EGL context unavailable: {e}"))?;
    let mut renderer=unsafe {GlesRenderer::new(context)}
        .map_err(|e|format!("GLES renderer unavailable: {e}"))?;

    for index in 0..frames {
        // Allocate and release each real DMA-BUF. This checks that the GPU
        // can re-render successive output-sized surfaces, not merely clear
        // one tiny 64x64 fixture.
        let mut buffer=allocator.create_buffer(
            width,height,Fourcc::Abgr8888,&[Modifier::Linear]
        ).map_err(|e|format!("DMA-BUF output allocation failed: {e}"))?;
        let mut framebuffer=renderer.bind(&mut buffer)
            .map_err(|e|format!("DMA-BUF output render target unavailable: {e}"))?;
        let mut frame=renderer.render(
            &mut framebuffer,(width as i32,height as i32).into(),Transform::Normal
        ).map_err(|e|format!("GLES output-sized frame start failed: {e}"))?;
        frame.clear(
            if index%2==0 {Color32F::new(0.07,0.28,0.67,1.0)}
            else {Color32F::new(0.17,0.38,0.27,1.0)},
            &[Rectangle::from_size((width as i32,height as i32).into())],
        ).map_err(|e|format!("GLES output-sized clear failed: {e}"))?;
        frame.finish()
            .map_err(|e|format!("GLES output-sized frame finish failed: {e}"))?
            .wait().map_err(|e|format!("GLES output-sized GPU sync failed: {e}"))?;
    }
    Ok(())
}

/// Exercise a genuinely exported DMA-BUF FD import, independently of the
/// compositor's CPU wl_shm upload path. The producer draws into one GBM
/// allocation, exports duplicated plane FDs with the actual offsets/strides,
/// drops its own allocation, then imports those FDs into the GLES renderer
/// and blits the resulting texture into a DIFFERENT GPU allocation.
///
/// This is an output-sized, offscreen libseat/VT-recovery probe only: the
/// buffer is generated by Melania, NOT supplied by a Wayland client. No
/// linux-dmabuf global is advertised, and no DRM ioctl presents these pixels.
/// All GBM/EGL/texture resources are released before the caller closes its
/// original libseat-owned DRM descriptor.
pub fn smoke_import_exported_dmabuf(
    fd: DrmDeviceFd, width: u32, height: u32,
) -> Result<(),String> {
    use smithay::backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags},
            dmabuf::{Dmabuf, DmabufAllocator, DmabufFlags},
            Allocator, Buffer, Fourcc, Modifier,
        },
        renderer::{Bind, Color32F, Frame, ImportDma, Renderer},
    };
    use smithay::utils::{Rectangle, Transform};

    // A source and destination exist simultaneously. Do not use the
    // single-frame budget to authorize twice as much GPU memory.
    validate_gbm_output_dimensions(width,height,2)?;
    let gbm=GbmDevice::new(fd)
        .map_err(|error|format!("DMA-BUF external import GBM unavailable: {error}"))?;
    // Independent allocators emulate producer/consumer buffer ownership.
    let mut producer=DmabufAllocator(GbmAllocator::new(
        gbm.clone(),GbmBufferFlags::RENDERING,
    ));
    let mut consumer=DmabufAllocator(GbmAllocator::new(
        gbm.clone(),GbmBufferFlags::RENDERING,
    ));
    let mut source=producer.create_buffer(
        width,height,Fourcc::Abgr8888,&[Modifier::Linear],
    ).map_err(|error|format!("DMA-BUF producer allocation failed: {error}"))?;
    let mut destination=consumer.create_buffer(
        width,height,Fourcc::Abgr8888,&[Modifier::Linear],
    ).map_err(|error|format!("DMA-BUF consumer allocation failed: {error}"))?;
    let display=unsafe {EGLDisplay::new(gbm)}
        .map_err(|error|format!("DMA-BUF external import EGL unavailable: {error}"))?;
    let context=EGLContext::new(&display)
        .map_err(|error|format!("DMA-BUF external import EGL context failed: {error}"))?;
    let mut renderer=unsafe {GlesRenderer::new(context)}
        .map_err(|error|format!("DMA-BUF external import GLES unavailable: {error}"))?;
    let output_rect=Rectangle::from_size((width as i32,height as i32).into());
    {
        let mut bound=renderer.bind(&mut source)
            .map_err(|error|format!("DMA-BUF producer framebuffer bind failed: {error}"))?;
        let mut frame=renderer.render(&mut bound,
            (width as i32,height as i32).into(),Transform::Normal
        ).map_err(|error|format!("DMA-BUF producer frame start failed: {error}"))?;
        frame.clear(Color32F::new(0.24,0.58,0.12,1.0),&[output_rect])
            .map_err(|error|format!("DMA-BUF producer draw failed: {error}"))?;
        frame.finish().map_err(|error|format!("DMA-BUF producer frame finish failed: {error}"))?
            .wait().map_err(|error|format!("DMA-BUF producer GPU sync failed: {error}"))?;
    }

    // Rebuild an independently owned DMA-BUF from dup() of its real kernel
    // plane descriptors. Merely cloning the Rust Arc would NOT exercise the
    // external-FD import lifetime that Wayland clients depend upon.
    let mut builder=Dmabuf::builder_from_buffer(&source,DmabufFlags::empty());
    let mut planes=0u32;
    for ((handle,offset),stride) in source.handles()
        .zip(source.offsets()).zip(source.strides()) {
        if stride==0 || !builder.add_plane(
            handle.try_clone_to_owned()
                .map_err(|error|format!("Cannot duplicate DMA-BUF plane FD: {error}"))?,
            planes,offset,stride,
        ) {
            return Err("DMA-BUF external import has an invalid plane table".into());
        }
        planes=planes.checked_add(1)
            .ok_or_else(||"DMA-BUF plane count overflow".to_string())?;
    }
    if planes==0 || planes>4 {
        return Err("DMA-BUF external import requires 1 to 4 real planes".into());
    }
    let exported=builder.build()
        .ok_or_else(||"DMA-BUF external import has no exported planes".to_string())?;
    drop(source); // The cloned kernel FDs, not the original BO, own the import.

    if !renderer.has_dmabuf_format(exported.format()) {
        return Err("GPU does not advertise the exported DMA-BUF format/modifier for texture import".into());
    }
    let texture=renderer.import_dmabuf(&exported,None)
        .map_err(|error|format!("Real EGL DMA-BUF FD texture import failed: {error}"))?;
    {
        let mut bound=renderer.bind(&mut destination)
            .map_err(|error|format!("DMA-BUF consumer framebuffer bind failed: {error}"))?;
        let mut frame=renderer.render(&mut bound,
            (width as i32,height as i32).into(),Transform::Normal
        ).map_err(|error|format!("DMA-BUF consumer frame start failed: {error}"))?;
        let src=Rectangle::from_size((width as f64,height as f64).into());
        frame.clear(Color32F::new(0.0,0.0,0.0,1.0),&[output_rect])
            .map_err(|error|format!("DMA-BUF consumer clear failed: {error}"))?;
        frame.render_texture_from_to(
            &texture,src,output_rect,&[output_rect],&[],
            Transform::Normal,1.0,None,&[],
        ).map_err(|error|format!("DMA-BUF external texture blit failed: {error}"))?;
        frame.finish().map_err(|error|format!("DMA-BUF consumer finish failed: {error}"))?
            .wait().map_err(|error|format!("DMA-BUF imported texture GPU sync failed: {error}"))?;
    }
    Ok(())
}

/// Stage 5 initialization is capability discovery only: presentation is
/// deliberately disabled even if EGL/GLES setup succeeds on the GPU.
pub fn native_scanout_is_implemented() -> bool {false}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_sized_gpu_probe_limits_gpu_memory_and_oversized_edid() {
        assert_eq!(validate_gbm_output_dimensions(1280,800,2),
                   Ok(1280*800*4*2));
        assert!(validate_gbm_output_dimensions(0,800,2).is_err());
        assert!(validate_gbm_output_dimensions(8192,4320,2).is_err());
        assert!(validate_gbm_output_dimensions(4096,4096,2).is_err());
        assert!(validate_gbm_output_dimensions(1920,1080,4).is_err());
        assert!(validate_gbm_output_dimensions(1920,1080,0).is_err());
        assert!(!native_scanout_is_implemented());
    }

    #[test]
    fn renderer_capability_does_not_claim_a_full_compositor() {
        assert!(!native_scanout_is_implemented());
        // A capability report contains hardware facts only; no mode list or
        // fabricated XDG session ID is persisted here.
        let result=GpuRenderCapabilities {
            gbm_backend:"mesa".into(),
            dmabuf_render_formats:7,
        };
        assert_eq!(result.dmabuf_render_formats,7);
    }
}
