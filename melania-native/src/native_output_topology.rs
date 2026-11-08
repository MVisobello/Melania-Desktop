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
    connector, crtc, encoder, framebuffer, Device as ControlDevice,
};

pub const MAX_ACTIVE_DRM_OUTPUTS:usize=8;
const MAX_CONNECTORS:usize=32;
const MAX_CRTCS:usize=32;
const MAX_OUTPUT_SIDE:u16=16384;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ActiveOutputRoute {
    pub connector:connector::Handle,
    pub crtc:crtc::Handle,
    /// Preserve the exact kernel encoder identity across both complete scans.
    /// Matching CRTC / FB_ID alone does not prove that a route stayed owned.
    pub original_encoder:encoder::Handle,
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
/// may be implicitly shared by two outputs and no connector or encoder
/// may appear twice. Display mirroring via cloned CRTCs is intentionally unsupported
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
            old.connector==route.connector || old.crtc==route.crtc ||
            old.original_encoder==route.original_encoder
        }) {
            return Err("DRM topology has duplicate connector/encoder or shared CRTC".into());
        }
    }
    Ok(())
}

/// Resource handles are kernel object IDs. Ignore enumeration order but
/// refuse duplicate IDs, which would otherwise hide a malformed inventory.
fn same_resource_handles<T:Eq>(before:&[T],after:&[T])->bool {
    if before.len()!=after.len() {return false;}
    // drm-rs connector::Handle and crtc::Handle implement Eq, but NOT Ord.
    // Compare bounded (<=32) kernel object-ID sets without sorting or
    // converting handles into fabricated IDs. Reject duplicates on EITHER
    // read, including two identical malformed inventories.
    for (i,handle) in before.iter().enumerate() {
        if before[..i].contains(handle) || !after.contains(handle) {
            return false;
        }
    }
    for (i,handle) in after.iter().enumerate() {
        if after[..i].contains(handle) {return false;}
    }
    true
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
        // The connector/encoder/CRTC reads above are separate kernel ioctls.
        // A VT change or an external DRM master may alter the route during
        // this single inventory pass even if its connector and FB_ID appear
        // unchanged in the next full topology pass. Reject a mixed snapshot
        // before ever describing it as a stable output.
        let latest_crtc=card.get_crtc(crtc_handle)
            .map_err(|e|format!("DRM second active CRTC read failed: {e}"))?;
        if latest_crtc.framebuffer()!=Some(original_fb) ||
           latest_crtc.mode()!=Some(mode) ||
           latest_crtc.position()!=crtc_info.position() {
            return Err("DRM active CRTC changed during topology snapshot".into());
        }
        let latest_connector=card.get_connector(connector_handle,false)
            .map_err(|e|format!("DRM second active connector read failed: {e}"))?;
        if latest_connector.state()!=connector::State::Connected ||
           latest_connector.current_encoder()!=Some(encoder_handle) {
            return Err("DRM connector or encoder changed during topology snapshot".into());
        }
        let latest_encoder=card.get_encoder(encoder_handle)
            .map_err(|e|format!("DRM second attached encoder read failed: {e}"))?;
        if latest_encoder.crtc()!=Some(crtc_handle) {
            return Err("DRM attached encoder moved during topology snapshot".into());
        }
        let (width,height)=mode.size();
        active.push(ActiveOutputRoute{
            connector:connector_handle,
            crtc:crtc_handle,
            original_encoder:encoder_handle,
            original_fb,width,height,
        });
        if active.len()>MAX_ACTIVE_DRM_OUTPUTS {
            return Err("DRM topology exceeds supported active output capacity".into());
        }
    }
    // Re-read the kernel resource inventory after traversing ALL outputs.
    // A hotplug or VT takeover can add/remove a connector or CRTC during
    // the scan, even if the previously observed active routes still match.
    // An incomplete resource snapshot must never authorize the first KMS
    // frame. Compare handle SETS (not kernel enumeration order), and reject
    // duplicate handles rather than silently deduplicating them.
    let latest_resources=card.resource_handles()
        .map_err(|e|format!("DRM final output resource list unavailable: {e}"))?;
    if !same_resource_handles(resources.connectors(),latest_resources.connectors()) ||
       !same_resource_handles(resources.crtcs(),latest_resources.crtcs()) {
        return Err("DRM connector/CRTC inventory changed during topology snapshot".into());
    }
    active.sort_by_key(|route|format!("{:?}",route.connector));
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
    use drm::control;
    fn route(conn:u32,scanout:u32,fb:u32,w:u16,hgt:u16)->ActiveOutputRoute {
        ActiveOutputRoute {
            connector:control::from_u32(conn).unwrap(),
            crtc:control::from_u32(scanout).unwrap(),
            original_encoder:control::from_u32(conn+1000).unwrap(),
            original_fb:control::from_u32(fb).unwrap(),
            width:w,height:hgt,
        }
    }
    #[test]
    fn kernel_resource_inventory_rejects_hotplug_and_duplicate_ids() {
        assert!(same_resource_handles(&[3u32,1,2],&[2,3,1]));
        assert!(!same_resource_handles(&[1u32,2],&[1,2,3]));
        assert!(!same_resource_handles(&[1u32,2,3],&[1,2,4]));
        assert!(!same_resource_handles(&[1u32,1,2],&[1,2,2]));
        assert!(!same_resource_handles(&[1u32,1],&[1,1]));
        assert!(same_resource_handles::<u32>(&[],&[]));
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
    fn encoder_takeover_between_complete_snapshots_is_detected() {
        // A connector can keep the same CRTC, FB_ID and dimensions after
        // external DRM ownership changes: the identity must still differ.
        let first=NativeOutputTopology{active:vec![route(11,21,31,1920,1080)]};
        let mut changed=first.clone();
        changed.active[0].original_encoder=control::from_u32(2999).unwrap();
        assert_ne!(first,changed);
        assert!(validate_active_routes(&changed.active).is_ok());
    }
    #[test]
    fn two_output_snapshot_rejects_encoder_reuse_with_distinct_crtcs() {
        let first=route(11,21,31,1920,1080);
        let mut second=route(12,22,32,1920,1080);
        second.original_encoder=first.original_encoder;
        assert!(validate_active_routes(&[first,second]).is_err());
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
