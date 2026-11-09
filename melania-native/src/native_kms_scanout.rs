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
    // A framebuffer handle alone is insufficient: the CRTC can be
    // deactivated or modeset while an old FB_ID remains attached.
    let Some(initial_mode) = current.mode() else {
        return Ok(false);
    };
    let initial_size = initial_mode.size();
    let initial_refresh = initial_mode.vrefresh();
    if initial_size.0 == 0 || initial_size.1 == 0 || initial_refresh == 0 ||
       current.framebuffer() != Some(expected_fb) {
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
    // Keep the proven route requirement explicit. The stable-route
    // recheck below must not turn a detached encoder into a frame ACK.
    if !(routed.crtc() == Some(crtc)) {
        return Ok(false);
    }
    // The first three ioctls are not an atomic DRM snapshot. Recheck
    // the *actual* FB_ID and route after reading the encoder, including
    // the same connector and encoder identity. Never ACK on a mixed
    // hotplug / VT-takeover snapshot.
    let latest_crtc = card.get_crtc(crtc)
        .map_err(|error| format!("DRM second get_crtc failed: {error}"))?;
    // Resolution and rounded refresh are not a unique DRM mode identity:
    // distinct pixel clocks / porch timings can share both. A modeset
    // during the split kernel reads must invalidate this frame ACK.
    // A VT takeover or another DRM master can move this same FB_ID and
    // exact mode to a different CRTC origin between non-atomic ioctls.
    // Do not ACK a frame when its visible placement has changed.
    if latest_crtc.framebuffer() != Some(expected_fb) ||
       latest_crtc.mode() != Some(initial_mode) ||
       latest_crtc.position() != current.position() {
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
    if latest_encoder.crtc() != Some(crtc) {
        return Ok(false);
    }
    // The second CRTC read happened BEFORE the second connector/encoder
    // reads. Those ioctls are not an atomic kernel snapshot: a foreign DRM
    // master can modeset or reposition the same FB_ID during that gap.
    // Recheck the CRTC *after* the final encoder read, before returning the
    // verified frame ACK to Wayland. No extra frame or GPU allocation.
    let final_crtc = card.get_crtc(crtc)
        .map_err(|error| format!("DRM final get_crtc failed: {error}"))?;
    Ok(final_crtc.framebuffer() == Some(expected_fb) &&
       final_crtc.mode() == Some(initial_mode) &&
       final_crtc.position() == current.position())
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
    // An identical size is not an identical DRM mode: a different pixel
    // clock or sync timing can appear after a concurrent modeset. Capture
    // the complete first kernel mode and require the same mode on re-read.
    let Some(initial_mode)=current.mode() else {return Ok(false);};
    // A CRTC carrying a stale FB_ID is not an active scanout if its
    // kernel mode is malformed (zero dimensions or zero refresh).
    // Reject before trusting the connector route or reusing the buffer.
    let size=initial_mode.size();
    if current.framebuffer()!=Some(expected_fb) ||
       size.0==0 || size.1==0 ||
       initial_mode.size()!=expected_size || initial_mode.vrefresh()==0 {
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
    // An external modeset can reposition the same FB_ID/mode on the CRTC
    // without changing either handle. Such a split snapshot is not owned.
    if latest_crtc.framebuffer()!=Some(expected_fb) ||
       latest_crtc.mode()!=Some(initial_mode) ||
       latest_crtc.position()!=current.position() {
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
    if latest_routing.crtc()!=Some(crtc) {
        return Ok(false);
    }
    // The second CRTC read predates the final connector/encoder ioctls.
    // A VT takeover or external modeset can move the same framebuffer and
    // complete mode during that gap. Refuse ownership if the final kernel
    // CRTC snapshot differs; never write the next owned KMS buffer on a
    // stale route. This is still not an atomic DRM master/lease guarantee.
    let final_crtc=card.get_crtc(crtc)
        .map_err(|error|format!("DRM owner final get_crtc failed: {error}"))?;
    Ok(final_crtc.framebuffer()==Some(expected_fb) &&
       final_crtc.mode()==Some(initial_mode) &&
       final_crtc.position()==current.position())
}


/// Verify that the SAME connector encoder remains attached across a
/// complete KMS frame, not only during eventual scanout rollback. Another
/// encoder may map to the same CRTC after a VT/hotplug takeover; do not ACK
/// that frame or release Wayland client buffers as if still owned.
///
/// Read the exact original route before and after the existing two-pass
/// framebuffer/mode verification, via genuine drm-rs kernel ioctls.
/// This is not an atomic DRM lease or a physical VT acceptance test.
pub fn verify_original_encoder_frame(
    card:&impl ControlDevice,
    crtc:crtc::Handle,
    connector:connector::Handle,
    original_encoder:drm::control::encoder::Handle,
    expected_fb:framebuffer::Handle,
    expected_size:(u16,u16),
)->Result<bool,String> {
    fn original_route(
        card:&impl ControlDevice,
        crtc:crtc::Handle,
        connector:connector::Handle,
        original_encoder:drm::control::encoder::Handle,
    )->Result<bool,String> {
        let output=card.get_connector(connector,false)
            .map_err(|e|format!("DRM original-encoder frame connector ioctl failed: {e}"))?;
        if output.state()!=connector::State::Connected ||
           output.current_encoder()!=Some(original_encoder) {
            return Ok(false);
        }
        let routed=card.get_encoder(original_encoder)
            .map_err(|e|format!("DRM original-encoder frame encoder ioctl failed: {e}"))?;
        Ok(routed.crtc()==Some(crtc))
    }
    // Begin a frame-spanning CRTC baseline BEFORE the first original-route
    // read. The inner presenter check is not an atomic DRM transaction:
    // a VT takeover can restore the same connector/encoder identity while
    // changing CRTC mode or position after its internal second ioctl.
    let first_crtc=card.get_crtc(crtc)
        .map_err(|e|format!("DRM original-encoder frame baseline CRTC ioctl failed: {e}"))?;
    let Some(first_mode)=first_crtc.mode() else {return Ok(false);};
    if first_crtc.framebuffer()!=Some(expected_fb) ||
       first_mode.size()!=expected_size {
        return Ok(false);
    }
    if !original_route(card,crtc,connector,original_encoder)? {
        return Ok(false);
    }
    if !verify_presenter_ownership(card,crtc,connector,expected_fb,expected_size)? {
        return Ok(false);
    }
    if !original_route(card,crtc,connector,original_encoder)? {
        return Ok(false);
    }
    // Close the outer bracket AFTER the final encoder read. Even with the
    // exact original connector route still attached, never acknowledge a
    // framebuffer that was moved or modeset between non-atomic read passes.
    let final_crtc=card.get_crtc(crtc)
        .map_err(|e|format!("DRM original-encoder frame final CRTC ioctl failed: {e}"))?;
    Ok(final_crtc.framebuffer()==Some(expected_fb) &&
       final_crtc.mode()==Some(first_mode) &&
       final_crtc.position()==first_crtc.position())
}

/// Pin the complete original DRM mode and CRTC origin for every frame
/// readback. A foreign modeset may preserve connector, encoder, FB_ID and
/// resolution while changing refresh, sync timings or visible placement.
/// Such a frame must never be acknowledged or used for another KMS ioctl.
/// These are genuine kernel CRTC reads, not an atomic DRM lease.
pub fn verify_original_encoder_frame_at_baseline(
    card:&impl ControlDevice,
    crtc:crtc::Handle,
    connector:connector::Handle,
    original_encoder:drm::control::encoder::Handle,
    expected_fb:framebuffer::Handle,
    expected_size:(u16,u16),
    expected_mode:drm::control::Mode,
    expected_position:(u32,u32),
)->Result<bool,String> {
    let before=card.get_crtc(crtc)
        .map_err(|e|format!("DRM baseline frame first CRTC ioctl failed: {e}"))?;
    if before.mode()!=Some(expected_mode) ||
       before.position()!=expected_position ||
       before.framebuffer()!=Some(expected_fb) {
        return Ok(false);
    }
    if !verify_original_encoder_frame(
        card,crtc,connector,original_encoder,expected_fb,expected_size
    )? {
        return Ok(false);
    }
    // Close the bracket after the original encoder's final kernel read.
    // Do not accept a same-size foreign mode/position transition between
    // independent DRM ioctls or a changed FB_ID after the route check.
    let after=card.get_crtc(crtc)
        .map_err(|e|format!("DRM baseline frame final CRTC ioctl failed: {e}"))?;
    Ok(after.mode()==Some(expected_mode) &&
       after.position()==expected_position &&
       after.framebuffer()==Some(expected_fb))
}

/// Validate the original kernel connector -> encoder -> CRTC route before
/// restoring an owned framebuffer and after the restore ioctl completes.
/// Linux may move a connector to another encoder targeting the SAME CRTC;
/// accepting only the CRTC would overwrite a foreign scanout. Require the
/// original encoder, full DRM mode, position and permitted FB_ID, in TWO
/// independent kernel-read passes. This is not an atomic snapshot or DRM
/// master lease, so physical VT/hotplug validation remains mandatory.
///
/// The owner may pass its currently allocated FB_IDs before restoration;
/// passing [] after set_crtc permits ONLY the original framebuffer.
pub fn verify_rollback_route(
    card:&impl ControlDevice,
    original:&crtc::Info,
    connector:connector::Handle,
    original_encoder:drm::control::encoder::Handle,
    owned_fbs:&[framebuffer::Handle],
)->Result<bool,String> {
    // A saved CRTC with no active video mode cannot be used as proof that
    // rollback is safe. A stale framebuffer handle may survive a VT switch
    // while the output is disabled. Fail closed before ANY restore modeset,
    // even if two kernel snapshots happen to agree on that inactive state.
    let Some(original_mode)=original.mode() else {return Ok(false);};
    // A restorable active scanout requires an original framebuffer. Without
    // one, a post-restore CRTC can retain an owned FB and still satisfy the
    // old route predicate when the caller passes it among owned_fbs.
    // Never enter a KMS rollback modeset without a concrete saved FB_ID.
    if original.framebuffer().is_none() {return Ok(false);}
    let original_size=original_mode.size();
    if original_size.0==0 || original_size.1==0 || original_mode.vrefresh()==0 {
        return Ok(false);
    }
    fn snapshot(
        card:&impl ControlDevice,
        original:&crtc::Info,
        connector:connector::Handle,
        expected_encoder:drm::control::encoder::Handle,
        owned_fbs:&[framebuffer::Handle],
    )->Result<Option<framebuffer::Handle>,String> {
        let current=card.get_crtc(original.handle())
            .map_err(|e|format!("DRM rollback get_crtc failed: {e}"))?;
        if current.mode()!=original.mode() || current.position()!=original.position() {
            return Ok(None);
        }
        let Some(fb)=current.framebuffer() else {return Ok(None)};
        if original.framebuffer()!=Some(fb) && !owned_fbs.contains(&fb) {
            return Ok(None);
        }
        let output=card.get_connector(connector,false)
            .map_err(|e|format!("DRM rollback get_connector failed: {e}"))?;
        if output.state()!=connector::State::Connected ||
            output.current_encoder()!=Some(expected_encoder) {
            return Ok(None);
        }
        let encoder=card.get_encoder(expected_encoder)
            .map_err(|e|format!("DRM rollback get_encoder failed: {e}"))?;
        if encoder.crtc()!=Some(original.handle()) {
            return Ok(None);
        }
        Ok(Some(fb))
    }
    let first=snapshot(card,original,connector,original_encoder,owned_fbs)?;
    if first.is_none() {return Ok(false);}
    let second=snapshot(card,original,connector,original_encoder,owned_fbs)?;
    if first!=second {return Ok(false);}
    // Each snapshot finishes at get_encoder, AFTER its last CRTC read.
    // A concurrent VT handoff/modeset can change FB_ID, complete DRM mode
    // or CRTC origin in that gap while retaining the original encoder.
    // Re-read the actual kernel CRTC after the last route ioctl, for both
    // pre-restore and post-restore checks in CPU and GBM Restore guards.
    // This cannot substitute for atomic ownership or hardware VT testing.
    let final_crtc=card.get_crtc(original.handle())
        .map_err(|e|format!("DRM rollback final get_crtc failed: {e}"))?;
    Ok(final_crtc.framebuffer()==first &&
       final_crtc.mode()==original.mode() &&
       final_crtc.position()==original.position())
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
    fn rollback_requires_saved_framebuffer_before_kernel_restore() {
        // This precondition must run before the first kernel snapshot and
        // therefore before either CPU or GBM Restore guard calls set_crtc.
        let source=include_str!("native_kms_scanout.rs");
        let verifier=source.split("pub fn verify_rollback_route(").nth(1).unwrap()
            .split("/// Pure snapshot predicate").next().unwrap();
        let missing=verifier.find("if original.framebuffer().is_none() {return Ok(false);}").unwrap();
        let first_read=verifier.find("let first=snapshot(").unwrap();
        assert!(missing<first_read);
    }

    #[test]
    fn kernel_scanout_ack_requires_unchanged_crtc_position() {
        // The production verifier uses two real drm-rs CRTC ioctl reads.
        // A same-mode/same-FB position takeover must not acknowledge a frame.
        let source=include_str!("native_kms_scanout.rs");
        let verifier=source.split("pub fn verify_kernel_scanout(").nth(1).unwrap()
            .split("pub fn verify_presenter_ownership(").next().unwrap();
        assert!(verifier.contains("latest_crtc.position() != current.position()"));
    }
    #[test]
    fn presenter_rejects_inactive_zero_refresh_modes_before_reuse() {
        // Guard the production DRM ioctl path rather than a fabricated
        // success predicate: a stale framebuffer handle is insufficient.
        let source=include_str!("native_kms_scanout.rs");
        let verifier=source.split("pub fn verify_presenter_ownership(").nth(1).unwrap()
            .split("pub fn verify_original_encoder_frame(").next().unwrap();
        assert!(verifier.contains("size.0==0 || size.1==0"));
        assert!(verifier.contains("initial_mode.vrefresh()==0"));
        assert!(verifier.find("initial_mode.vrefresh()==0").unwrap()
            < verifier.find("card.get_connector(connector,false)").unwrap());
    }
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
