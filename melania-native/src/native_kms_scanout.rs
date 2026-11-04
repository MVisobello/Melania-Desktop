//! Stage96: kernel-backed CRTC + connector route verification.
//!
//! This function reads the *actual* DRM kernel state on the FD provided by
//! the caller. It does not acquire DRM master, change a mode, keep the old
//! framebuffer alive, authenticate libseat, or establish native login.
//!
//! A successful return is only evidence of the expected scanout at the time
//! of these ioctl reads. The caller must independently hold the current
//! libseat resource epoch, own the submitted framebuffer, and reject stale
//! Wayland frame tickets before releasing clients.

use drm::control::{connector, crtc, framebuffer, Device as ControlDevice};

/// Check that the kernel reports BOTH the expected framebuffer on the CRTC
/// and an active connector encoder routing to that CRTC.
///
/// False is returned for a missing encoder, a moved connector or an old FB_ID.
/// Any DRM ioctl error is propagated; callers must fail closed, not fabricate
/// an acknowledgement after losing the GPU or seat.
pub fn verify_kernel_scanout(
    card: &impl ControlDevice,
    crtc: crtc::Handle,
    connector: connector::Handle,
    expected_fb: framebuffer::Handle,
) -> Result<bool, String> {
    let current = card.get_crtc(crtc)
        .map_err(|error| format!("DRM get_crtc failed: {error}"))?;
    if current.framebuffer() != Some(expected_fb) {
        return Ok(false);
    }
    let attached = card.get_connector(connector, false)
        .map_err(|error| format!("DRM get_connector failed: {error}"))?;
    let Some(encoder) = attached.current_encoder() else {
        return Ok(false);
    };
    let routed = card.get_encoder(encoder)
        .map_err(|error| format!("DRM get_encoder failed: {error}"))?;
    Ok(routed.crtc() == Some(crtc))
}
