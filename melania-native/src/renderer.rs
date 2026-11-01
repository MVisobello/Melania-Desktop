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
pub fn smoke_render_gbm_offscreen(fd: DrmDeviceFd) -> Result<(),String> {
    use smithay::backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags},
            dmabuf::DmabufAllocator,
            Allocator, Fourcc, Modifier,
        },
        renderer::{Bind, Color32F, Frame, Renderer},
    };
    use smithay::utils::{Rectangle,Transform};

    const SIZE: u32=64;
    let gbm=GbmDevice::new(fd).map_err(|e|format!("GBM offscreen unavailable: {e}"))?;
    let mut allocator=DmabufAllocator(GbmAllocator::new(gbm.clone(),GbmBufferFlags::RENDERING));
    let mut buffer=allocator.create_buffer(SIZE,SIZE,Fourcc::Abgr8888,&[Modifier::Linear])
        .map_err(|e|format!("DMA-BUF offscreen allocation failed: {e}"))?;

    // Safety: the valid GBM device and DRM descriptor remain live until the
    // renderer and temporary DMA-BUF have completed the GPU synchronization.
    let display=unsafe {EGLDisplay::new(gbm)}
        .map_err(|e|format!("EGL display unavailable: {e}"))?;
    let context=EGLContext::new(&display)
        .map_err(|e|format!("EGL context unavailable: {e}"))?;
    let mut renderer=unsafe {GlesRenderer::new(context)}
        .map_err(|e|format!("GLES renderer unavailable: {e}"))?;
    let mut framebuffer=renderer.bind(&mut buffer)
        .map_err(|e|format!("DMA-BUF render target unavailable: {e}"))?;
    let mut frame=renderer.render(&mut framebuffer,(SIZE as i32,SIZE as i32).into(),Transform::Normal)
        .map_err(|e|format!("GLES frame start failed: {e}"))?;
    frame.clear(
        Color32F::new(0.07,0.28,0.67,1.0),
        &[Rectangle::from_size((SIZE as i32,SIZE as i32).into())],
    ).map_err(|e|format!("GLES clear failed: {e}"))?;
    frame.finish()
        .map_err(|e|format!("GLES finish failed: {e}"))?
        .wait().map_err(|e|format!("GLES synchronization failed: {e}"))?;
    Ok(())
}

/// Stage 5 initialization is capability discovery only: presentation is
/// deliberately disabled even if EGL/GLES setup succeeds on the GPU.
pub fn native_scanout_is_implemented() -> bool {false}

#[cfg(test)]
mod tests {
    use super::*;
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
