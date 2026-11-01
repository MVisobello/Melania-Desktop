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
        dmabuf_render_formats:display.dmabuf_render_formats().len(),
    })
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
