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
    // An encoder handle can remain attached while a monitor is disconnected.
    // Do not ACK a frame that is no longer visible on a connected output.
    if attached.state() != connector::State::Connected {
        return Ok(false);
    }
    let Some(encoder) = attached.current_encoder() else {
        return Ok(false);
    };
    let routed = card.get_encoder(encoder)
        .map_err(|error| format!("DRM get_encoder failed: {error}"))?;
    if routed.crtc() != Some(crtc) {
        return Ok(false);
    }
    // The first three ioctls are not an atomic DRM snapshot. Recheck
    // the *actual* FB_ID and route after reading the encoder, including
    // the same connector and encoder identity. Never ACK on a mixed
    // hotplug / VT-takeover snapshot.
    let latest_crtc = card.get_crtc(crtc)
        .map_err(|error| format!("DRM second get_crtc failed: {error}"))?;
    if latest_crtc.framebuffer() != Some(expected_fb) {
        return Ok(false);
    }
    let latest_connector = card.get_connector(connector, false)
        .map_err(|error| format!("DRM second get_connector failed: {error}"))?;
    if latest_connector.state() != connector::State::Connected ||
       latest_connector.current_encoder() != Some(encoder) {
        return Ok(false);
    }
    let latest_encoder = card.get_encoder(encoder)
        .map_err(|error| format!("DRM second get_encoder failed: {error}"))?;
    Ok(latest_encoder.crtc() == Some(crtc))
}

/// Validate the scanout owner *before* overwriting its next buffer or issuing
/// another KMS ioctl. A successful previous ACK is not a lease: another DRM
/// master, VT transition or hotplug can replace the connector route/mode/FB.
/// A mismatch must prevent a new frame from being submitted or ACKed.
pub fn verify_presenter_ownership(
    card: &impl ControlDevice,
    crtc: crtc::Handle,
    connector: connector::Handle,
    expected_fb: framebuffer::Handle,
    expected_size: (u16,u16),
) -> Result<bool,String> {
    let current=card.get_crtc(crtc)
        .map_err(|error|format!("DRM owner get_crtc failed: {error}"))?;
    if current.framebuffer()!=Some(expected_fb) ||
       current.mode().map(|mode|mode.size())!=Some(expected_size) {
        return Ok(false);
    }
    let output=card.get_connector(connector,false)
        .map_err(|error|format!("DRM owner get_connector failed: {error}"))?;
    if output.state()!=connector::State::Connected {
        return Ok(false);
    }
    let Some(encoder)=output.current_encoder() else {return Ok(false);};
    let routing=card.get_encoder(encoder)
        .map_err(|error|format!("DRM owner get_encoder failed: {error}"))?;
    if routing.crtc()!=Some(crtc) {return Ok(false);}

    // Separate DRM ioctls are not an atomic snapshot. A hotplug or VT
    // takeover can change the CRTC while its connector is being inspected.
    // Re-read all three kernel objects before accepting the route.
    let latest_crtc=card.get_crtc(crtc)
        .map_err(|error|format!("DRM owner second get_crtc failed: {error}"))?;
    if latest_crtc.framebuffer()!=Some(expected_fb) ||
       latest_crtc.mode().map(|mode|mode.size())!=Some(expected_size) {
        return Ok(false);
    }
    let latest_output=card.get_connector(connector,false)
        .map_err(|error|format!("DRM owner second get_connector failed: {error}"))?;
    if latest_output.state()!=connector::State::Connected ||
       latest_output.current_encoder()!=Some(encoder) {
        return Ok(false);
    }
    let latest_routing=card.get_encoder(encoder)
        .map_err(|error|format!("DRM owner second get_encoder failed: {error}"))?;
    Ok(latest_routing.crtc()==Some(crtc))
}

/// Pure snapshot predicate for regression tests and for consumers that
/// already read the DRM resources. No virtual GPU or fabricated event is
/// treated as a physical acceptance test.
pub fn owner_snapshot_matches(
    observed_fb:Option<framebuffer::Handle>,
    observed_size:Option<(u16,u16)>,
    connector_connected:bool,
    observed_crtc:Option<crtc::Handle>,
    expected_fb:framebuffer::Handle,
    expected_size:(u16,u16),
    expected_crtc:crtc::Handle,
)->bool {
    observed_fb==Some(expected_fb) &&
    observed_size==Some(expected_size) &&
    connector_connected &&
    observed_crtc==Some(expected_crtc)
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use drm::control;
    #[test]
    fn denies_hotplug_modeset_and_external_fb_takeover() {
        let fb=control::from_u32(31).unwrap();
        let other=control::from_u32(32).unwrap();
        let crtc=control::from_u32(41).unwrap();
        let moved=control::from_u32(42).unwrap();
        let valid=||owner_snapshot_matches(Some(fb),Some((1280,800)),true,Some(crtc),fb,(1280,800),crtc);
        assert!(valid());
        assert!(!owner_snapshot_matches(Some(other),Some((1280,800)),true,Some(crtc),fb,(1280,800),crtc));
        assert!(!owner_snapshot_matches(Some(fb),Some((1920,1080)),true,Some(crtc),fb,(1280,800),crtc));
        assert!(!owner_snapshot_matches(Some(fb),Some((1280,800)),false,Some(crtc),fb,(1280,800),crtc));
        assert!(!owner_snapshot_matches(Some(fb),Some((1280,800)),true,Some(moved),fb,(1280,800),crtc));
    }
}
