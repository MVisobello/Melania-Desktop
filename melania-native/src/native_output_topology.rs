//! Read-only, seat-owned DRM output routing snapshot.
//!
//! COSMIC's broad architecture keeps a per-GPU backend with per-connector
//! outputs and per-CRTC scanout surfaces. Melania follows this separation
//! with original Rust and public drm-rs/Smithay APIs, never copied source.
//!
//! This module only READS the kernel topology through an existing libseat
//! DRM descriptor. It never modesets, creates buffers, claims DRM master or
//! grants native-login permission. A stale/missing route must be refused
//! before KMS presents the first framebuffer.

use drm::control::{
    self, connector, crtc, framebuffer, Device as ControlDevice,
};

pub const MAX_ACTIVE_DRM_OUTPUTS:usize=8;
const MAX_CONNECTORS:usize=32;
const MAX_CRTCS:usize=32;
const MAX_OUTPUT_SIDE:u16=16384;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ActiveOutputRoute {
    pub connector:connector::Handle,
    pub crtc:crtc::Handle,
    /// Borrowed kernel FB_ID, not an owned GEM/GBM framebuffer resource.
    pub original_fb:framebuffer::Handle,
    pub width:u16,
    pub height:u16,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct NativeOutputTopology {
    pub active:Vec<ActiveOutputRoute>,
}

impl NativeOutputTopology {
    /// QEMU's one-CRTC presenter still explicitly rejects multiple outputs.
    /// A production backend will instead create one scanout owner PER route.
    pub fn single_active(&self)->Result<ActiveOutputRoute,String> {
        match self.active.as_slice() {
            [route]=>Ok(*route),
            []=>Err("DRM card has no active connector/CRTC route".into()),
            _=>Err("single-output QEMU presenter cannot manage multiple active CRTCs".into()),
        }
    }
}

/// Validate invariants that a future per-output KMS owner needs: no CRTC
/// may be implicitly shared by two outputs and no connector may appear
/// twice. Display mirroring via cloned CRTCs is intentionally unsupported
/// until explicitly modeled; it must not accidentally share one swapchain.
pub fn validate_active_routes(routes:&[ActiveOutputRoute])->Result<(),String> {
    if routes.is_empty() {
        return Err("DRM topology contains no active scanout".into());
    }
    if routes.len()>MAX_ACTIVE_DRM_OUTPUTS {
        return Err("DRM topology exceeds supported active output capacity".into());
    }
    for (i,route) in routes.iter().enumerate() {
        if route.width==0 || route.height==0 ||
           route.width>MAX_OUTPUT_SIDE || route.height>MAX_OUTPUT_SIDE {
            return Err("DRM topology has invalid framebuffer dimensions".into());
        }
        if routes[..i].iter().any(|old|{
            old.connector==route.connector || old.crtc==route.crtc
        }) {
            return Err("DRM topology has duplicate connector or shared CRTC".into());
        }
    }
    Ok(())
}

/// Obtain the real currently-active connector -> encoder -> CRTC -> FB_ID
/// mapping, without guessing card0, a default connector, or a display mode.
///
/// Inaccessible active-resource reads fail closed. Disconnected connectors
/// and connected outputs without a currently attached encoder are dormant;
/// they are NOT treated as a working scanout or counted as ownership.
pub fn inspect_active_outputs(
    card:&impl ControlDevice,
)->Result<NativeOutputTopology,String> {
    let resources=card.resource_handles()
        .map_err(|e|format!("DRM output resource list unavailable: {e}"))?;
    if resources.connectors().len()>MAX_CONNECTORS ||
       resources.crtcs().len()>MAX_CRTCS {
        return Err("DRM topology unbounded connector/CRTC resource count".into());
    }
    let mut active=Vec::new();
    for &connector_handle in resources.connectors() {
        let info=card.get_connector(connector_handle,false)
            .map_err(|e|format!("DRM connector read failed: {e}"))?;
        if info.state()!=connector::State::Connected {continue;}
        let Some(encoder_handle)=info.current_encoder() else {continue;};
        let encoder=card.get_encoder(encoder_handle)
            .map_err(|e|format!("DRM attached encoder read failed: {e}"))?;
        let Some(crtc_handle)=encoder.crtc() else {continue;};
        if !resources.crtcs().contains(&crtc_handle) {
            return Err("DRM attached encoder routes outside discovered CRTC set".into());
        }
        let crtc_info=card.get_crtc(crtc_handle)
            .map_err(|e|format!("DRM active CRTC read failed: {e}"))?;
        let original_fb=crtc_info.framebuffer()
            .ok_or("DRM active output has no restorable original FB_ID")?;
        let mode=crtc_info.mode()
            .ok_or("DRM active output has no restorable original mode")?;
        let (width,height)=mode.size();
        active.push(ActiveOutputRoute{
            connector:connector_handle,
            crtc:crtc_handle,
            original_fb,width,height,
        });
        if active.len()>MAX_ACTIVE_DRM_OUTPUTS {
            return Err("DRM topology exceeds supported active output capacity".into());
        }
    }
    active.sort_by_key(|route|route.connector);
    validate_active_routes(&active)?;
    Ok(NativeOutputTopology{active})
}

/// Read the same kernel topology twice before rendering; reject a hotplug
/// or external modeset that moves a connector, changes mode or replaces
/// the baseline framebuffer between scans. The output owner must still
/// monitor libseat/udev and verify the kernel's state after each frame.
pub fn inspect_stable_active_outputs(
    card:&impl ControlDevice,
)->Result<NativeOutputTopology,String> {
    let before=inspect_active_outputs(card)?;
    let after=inspect_active_outputs(card)?;
    if before!=after {
        return Err("DRM topology changed while capturing scanout baseline".into());
    }
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(conn:u32,scanout:u32,fb:u32,w:u16,hgt:u16)->ActiveOutputRoute {
        ActiveOutputRoute {
            connector:control::from_u32(conn).unwrap(),
            crtc:control::from_u32(scanout).unwrap(),
            original_fb:control::from_u32(fb).unwrap(),
            width:w,height:hgt,
        }
    }
    #[test]
    fn two_independent_outputs_keep_unique_crtcs_and_borrowed_fbs() {
        let a=route(11,21,31,1920,1080);
        let b=route(12,22,32,2560,1440);
        assert!(validate_active_routes(&[a,b]).is_ok());
        assert_eq!(NativeOutputTopology{active:vec![a]}.single_active(),Ok(a));
        assert!(NativeOutputTopology{active:vec![a,b]}.single_active().is_err());
    }
    #[test]
    fn cloned_crtc_or_duplicate_connector_is_not_silently_shared() {
        let a=route(11,21,31,1920,1080);
        assert!(validate_active_routes(&[a,route(12,21,32,1920,1080)]).is_err());
        assert!(validate_active_routes(&[a,route(11,22,32,1920,1080)]).is_err());
    }
    #[test]
    fn rejects_empty_excessive_and_invalid_output_modes() {
        let first=route(1,101,201,1280,800);
        assert!(validate_active_routes(&[]).is_err());
        assert!(validate_active_routes(&[route(1,101,201,0,800)]).is_err());
        assert!(validate_active_routes(&[route(1,101,201,1280,0)]).is_err());
        assert!(validate_active_routes(&[first]).is_ok());
        let many=(0..9).map(|i|route(i+1,i+101,i+201,1920,1080))
            .collect::<Vec<_>>();
        assert!(validate_active_routes(&many).is_err());
    }
}
