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
