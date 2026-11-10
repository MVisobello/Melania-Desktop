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
const MAX_ENCODERS:usize=64;
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
       resources.crtcs().len()>MAX_CRTCS ||
       resources.encoders().len()>MAX_ENCODERS {
        return Err("DRM topology unbounded connector/CRTC/encoder resource count".into());
    }
    // Reject malformed initial inventories before any per-connector ioctl.
    // Comparing a snapshot with itself detects duplicate IDs, which a
    // before/after comparison alone would otherwise only reject at the end.
    if !same_resource_handles(resources.connectors(),resources.connectors()) ||
       !same_resource_handles(resources.crtcs(),resources.crtcs()) ||
       !same_resource_handles(resources.encoders(),resources.encoders()) {
        return Err("DRM initial resource inventory contains duplicate handles".into());
    }
    let mut active=Vec::new();
    for &connector_handle in resources.connectors() {
        let info=card.get_connector(connector_handle,false)
            .map_err(|e|format!("DRM connector read failed: {e}"))?;
        if info.state()!=connector::State::Connected {continue;}
        let Some(encoder_handle)=info.current_encoder() else {continue;};
        // Reject an encoder attached after the initial kernel inventory.
        // Mixed ioctl snapshots must never authorize a stale KMS route.
        if !resources.encoders().contains(&encoder_handle) {
            return Err("DRM attached encoder outside discovered encoder inventory".into());
        }
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
    // frame. Compare ALL three resource handle SETS: detached/standby
    // encoders also matter. An encoder can hotplug/disappear while its
    // connector and CRTC inventories remain unchanged; otherwise a later
    // modeset could acknowledge a frame against stale hardware topology.
    // Ignore kernel enumeration order, but reject duplicate resource IDs.
    let latest_resources=card.resource_handles()
        .map_err(|e|format!("DRM final output resource list unavailable: {e}"))?;
    // The final kernel inventory must obey the same resource bounds.
    // Do not scan an unbounded hotplug result, even if the original was safe.
    if latest_resources.connectors().len()>MAX_CONNECTORS ||
       latest_resources.crtcs().len()>MAX_CRTCS ||
       latest_resources.encoders().len()>MAX_ENCODERS {
        return Err("DRM final topology exceeds connector/CRTC/encoder limits".into());
    }
    if !same_resource_handles(resources.connectors(),latest_resources.connectors()) ||
       !same_resource_handles(resources.crtcs(),latest_resources.crtcs()) ||
       !same_resource_handles(resources.encoders(),latest_resources.encoders()) {
        return Err("DRM connector/CRTC/encoder inventory changed during topology snapshot".into());
    }
    active.sort_by_key(|route|format!("{:?}",route.connector));
    validate_active_routes(&active)?;
    Ok(NativeOutputTopology{active})
}

/// A second independent read of all active CRTCs. The high-level route
/// deliberately exposes only width/height for buffer planning, which cannot
/// distinguish a same-resolution refresh-rate/scan-timing modeset or a CRTC
/// position change. Retain the exact drm-rs Mode and position in this
/// ephemeral, read-only scanout signature; never use it to claim ownership.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct ActiveCrtcReadback<M> {
    crtc:crtc::Handle,
    framebuffer:framebuffer::Handle,
    mode:M,
    position:(u32,u32),
}

fn read_active_crtc_readbacks(
    card:&impl ControlDevice,
    topology:&NativeOutputTopology,
)->Result<Vec<ActiveCrtcReadback<drm::control::Mode>>,String> {
    let mut readbacks=Vec::with_capacity(topology.active.len());
    for route in &topology.active {
        let info=card.get_crtc(route.crtc)
            .map_err(|e|format!("DRM CRTC timing readback unavailable: {e}"))?;
        let framebuffer=info.framebuffer()
            .ok_or("DRM CRTC lost its original framebuffer during timing readback")?;
        let mode=info.mode()
            .ok_or("DRM CRTC lost its original mode during timing readback")?;
        if framebuffer!=route.original_fb ||
           mode.size()!=(route.width,route.height) {
            return Err("DRM CRTC framebuffer or mode dimensions changed during timing readback".into());
        }
        // GETCRTC x/y are viewport offsets inside THIS framebuffer.
        // Read its actual kernel dimensions; never confuse these offsets
        // with global monitor coordinates or assume two (0,0) CRTCs mirror.
        let fb_info=card.get_framebuffer(framebuffer)
            .map_err(|e|format!("DRM active framebuffer dimensions unreadable: {e}"))?;
        validate_crtc_framebuffer_viewport(
            mode.size(),info.position(),fb_info.size(),
        )?;
        readbacks.push(ActiveCrtcReadback {
            crtc:route.crtc,
            framebuffer,
            mode,
            position:info.position(),
        });
    }
    Ok(readbacks)
}

/// Per-CRTC scanout timing captured from real kernel readbacks. Unlike
/// ActiveOutputRoute (which stores only width/height and original FB_ID),
/// this preserves the *complete* drm-rs mode and CRTC origin across a VT
/// pause. It is read-only evidence, never a KMS ownership or login permit.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ActiveOutputTiming<M> {
    pub crtc:crtc::Handle,
    pub mode:M,
    pub position:(u32,u32),
}

/// A physical VT reacquisition cannot be certified if a previously active
/// output resumes with the same pixel dimensions but a different refresh
/// rate, pixel clock, sync timings, or screen origin. Compare the complete
/// mode, not merely mode.size(). The framebuffer ID is intentionally absent:
/// another seat owner may legitimately replace FB_ID during VT handoff.
pub fn verify_restored_output_timings<M:PartialEq>(
    before:&[ActiveOutputTiming<M>],
    after:&[ActiveOutputTiming<M>],
)->Result<(),String> {
    if before.is_empty() || before.len()!=after.len() ||
       before.len()>MAX_ACTIVE_DRM_OUTPUTS {
        return Err("Physical VT output timing set changed or is empty".into());
    }
    for (index,original) in before.iter().enumerate() {
        if before[..index].iter().any(|entry|entry.crtc==original.crtc) {
            return Err("Physical VT baseline has duplicate CRTC timing".into());
        }
        let mut matches=after.iter().filter(|entry|entry.crtc==original.crtc);
        let restored=matches.next()
            .ok_or("Physical VT lost the original CRTC timing")?;
        if matches.next().is_some() {
            return Err("Physical VT restored duplicate CRTC timings".into());
        }
        if restored.mode!=original.mode || restored.position!=original.position {
            return Err("Physical VT changed original CRTC mode timing or position".into());
        }
    }
    Ok(())
}

/// Legacy DRM GETCRTC x/y identify the viewport offset *into that
/// CRTC's framebuffer*, NOT the output's position in a desktop-wide layout.
/// Two independent physical outputs may validly both report (0,0).
/// Never interpret these offsets as proof of global monitor geometry.
///
/// Fail closed when a live CRTC tries to scan outside its own kernel FB.
/// This is a read-only, actual drm-rs GETFB preflight, not an atomic modeset,
/// a hardware-proven layout manager, or a DRM lease. Multi-output scene
/// placement needs a separate compositor-owned logical-output coordinate.
pub const MAX_SCANOUT_FRAMEBUFFER_SIDE:u32=32768;
pub fn validate_crtc_framebuffer_viewport(
    mode_size:(u16,u16),
    framebuffer_offset:(u32,u32),
    framebuffer_size:(u32,u32),
)->Result<(),String> {
    let (width,height)=(u32::from(mode_size.0),u32::from(mode_size.1));
    let (fb_width,fb_height)=framebuffer_size;
    if width==0 || height==0 ||
       width>u32::from(MAX_OUTPUT_SIDE) || height>u32::from(MAX_OUTPUT_SIDE) ||
       fb_width==0 || fb_height==0 ||
       fb_width>MAX_SCANOUT_FRAMEBUFFER_SIDE ||
       fb_height>MAX_SCANOUT_FRAMEBUFFER_SIDE {
        return Err("DRM CRTC or backing framebuffer has invalid dimensions".into());
    }
    let right=framebuffer_offset.0.checked_add(width)
        .ok_or("DRM CRTC framebuffer source X overflow")?;
    let bottom=framebuffer_offset.1.checked_add(height)
        .ok_or("DRM CRTC framebuffer source Y overflow")?;
    if right>fb_width || bottom>fb_height {
        return Err("DRM CRTC viewport exceeds the real kernel framebuffer dimensions".into());
    }
    Ok(())
}

/// The completed GBM rebuild is NOT proof if a queued libseat/udev event or an
/// external modeset changes scanout before the final VT acceptance marker.
/// Unlike a comparison across two VT owners, FB_ID must remain the same
/// within this *single* newly-acquired seat epoch. Fail closed on a missing,
/// duplicate or foreign CRTC timing even when the route lists look identical.
pub fn verify_unchanged_scanout_epoch<M:PartialEq>(
    before_routes:&NativeOutputTopology,
    before_timings:&[ActiveOutputTiming<M>],
    after_routes:&NativeOutputTopology,
    after_timings:&[ActiveOutputTiming<M>],
)->Result<(),String> {
    for (routes,timings) in [(before_routes,before_timings),(after_routes,after_timings)] {
        // Each live CRTC source viewport has already been checked
        // against its actual framebuffer during the two kernel scans.
        // Positions here remain FB-relative, not global monitor origins.
        validate_active_routes(&routes.active)?;
        if routes.active.len()!=timings.len() {
            return Err("DRM scanout timing count no longer matches active outputs".into());
        }
        for route in &routes.active {
            if timings.iter().filter(|t|t.crtc==route.crtc).count()!=1 {
                return Err("DRM scanout has missing, duplicate or foreign CRTC timing".into());
            }
        }
    }
    if before_routes!=after_routes || before_timings!=after_timings {
        return Err("DRM connector/encoder/CRTC/FB_ID or full mode and position changed after event dispatch".into());
    }
    Ok(())
}

/// Inspect every active route and the full kernel CRTC scanout timing in the
/// same two-pass stability transaction. Never combine a timing from one
/// topology epoch with connector/encoder IDs from another epoch.
pub fn inspect_stable_active_outputs_with_timings(
    card:&impl ControlDevice,
)->Result<(NativeOutputTopology,Vec<ActiveOutputTiming<drm::control::Mode>>),String> {
    let before=inspect_active_outputs(card)?;
    let before_crtcs=read_active_crtc_readbacks(card,&before)?;
    let after=inspect_active_outputs(card)?;
    let after_crtcs=read_active_crtc_readbacks(card,&after)?;
    if before!=after || before_crtcs!=after_crtcs {
        return Err("DRM route, CRTC timing or position changed while capturing scanout baseline".into());
    }
    let timings=after_crtcs.into_iter().map(|readback|ActiveOutputTiming {
        crtc:readback.crtc,
        mode:readback.mode,
        position:readback.position,
    }).collect();
    // Each pass validated real per-CRTC viewport bounds via GETFB;
    // two independent CRTCs at framebuffer offset (0,0) remain valid.
    Ok((after,timings))
}

/// Retain the existing stable-topology API for all existing KMS presenters,
/// which continue to use the exact same two-pass kernel validation.
pub fn inspect_stable_active_outputs(
    card:&impl ControlDevice,
)->Result<NativeOutputTopology,String> {
    inspect_stable_active_outputs_with_timings(card).map(|(topology,_)|topology)
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
    fn independent_real_crtcs_can_both_scan_out_at_framebuffer_origin_zero() {
        // On separate framebuffer IDs each CRTC's (0,0) means pixel zero
        // in ITS FB, not two physical outputs occupying one global area.
        let first=route(11,21,31,1920,1080);
        let second=route(12,22,32,2560,1440);
        let routes=NativeOutputTopology{active:vec![first,second]};
        let t1=ActiveOutputTiming{crtc:first.crtc,mode:60_000u32,position:(0,0)};
        let t2=ActiveOutputTiming{crtc:second.crtc,mode:60_000u32,position:(0,0)};
        assert!(validate_active_routes(&routes.active).is_ok());
        assert!(verify_unchanged_scanout_epoch(
            &routes,&[t1,t2],&routes,&[t2,t1]
        ).is_err()); // Order is still a separate epoch identity contract.
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(0,0),(1920,1080)).is_ok());
        assert!(validate_crtc_framebuffer_viewport((2560,1440),(0,0),(2560,1440)).is_ok());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(1920,0),(4480,1440)).is_ok());
    }

    #[test]
    fn real_fb_viewport_rejects_scanout_past_edge_or_illegal_dimensions() {
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(1,0),(1920,1080)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(0,1),(1920,1080)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(500,400),(2200,1400)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(280,320),(2200,1400)).is_ok());
        assert!(validate_crtc_framebuffer_viewport((0,1080),(0,0),(1920,1080)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,0),(0,0),(1920,1080)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(0,0),(0,1080)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1920,1080),(0,0),(1920,0)).is_err());
        assert!(validate_crtc_framebuffer_viewport(
            (1920,1080),(0,0),(MAX_SCANOUT_FRAMEBUFFER_SIDE+1,1080)
        ).is_err());
    }

    #[test]
    fn real_fb_viewport_refuses_untrusted_overflow_and_allows_shared_fb_panning() {
        assert!(validate_crtc_framebuffer_viewport((1280,720),(u32::MAX,0),(1280,720)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1280,720),(0,u32::MAX),(1280,720)).is_err());
        assert!(validate_crtc_framebuffer_viewport((1280,720),(1920,0),(3200,1080)).is_ok());
        assert!(validate_crtc_framebuffer_viewport((1280,720),(1921,0),(3200,1080)).is_err());
        // This is NOT a desktop origin: a framebuffer source offset may
        // be nonzero even when a physical monitor is positioned at (0,0).
        assert!(validate_crtc_framebuffer_viewport((800,600),(32,16),(1024,768)).is_ok());
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
    fn drm_encoder_handles_detect_dormant_encoder_hotplug_even_without_crtc_change() {
        let original:Vec<encoder::Handle> = [101u32,102,103].into_iter()
            .map(|id|control::from_u32(id).unwrap()).collect();
        let reordered=vec![original[2],original[0],original[1]];
        assert!(same_resource_handles(&original,&reordered));
        // A disconnected/standby encoder can disappear while the visible
        // connector/CRTC IDs remain unchanged: never accept that snapshot.
        assert!(!same_resource_handles(&original,&original[..2]));
        let mut replaced=original.clone();
        replaced[2]=control::from_u32(104).unwrap();
        assert!(!same_resource_handles(&original,&replaced));
        let duplicated=vec![original[0],original[0],original[2]];
        assert!(!same_resource_handles(&duplicated,&duplicated));
    }
    #[test]
    fn malformed_initial_resource_inventory_is_rejected_even_when_unchanged() {
        let duplicate_encoders:Vec<encoder::Handle>=[17u32,17].into_iter()
            .map(|id|control::from_u32(id).unwrap()).collect();
        assert!(!same_resource_handles(&duplicate_encoders,&duplicate_encoders));
        let distinct:Vec<encoder::Handle>=[17u32,18].into_iter()
            .map(|id|control::from_u32(id).unwrap()).collect();
        assert!(same_resource_handles(&distinct,&distinct));
    }
    #[test]
    fn real_vt_timing_gate_rejects_same_resolution_refresh_and_position_changes() {
        let crtc1=control::from_u32(21).unwrap();
        let crtc2=control::from_u32(22).unwrap();
        // u32 values represent the distinct full drm::control::Mode
        // signatures. Production passes drm-rs Mode, not an integer rate.
        let first=ActiveOutputTiming {crtc:crtc1,mode:60_000u32,position:(0,0)};
        let second=ActiveOutputTiming {crtc:crtc2,mode:60_000u32,position:(1920,0)};
        let baseline=[first,second];
        assert!(verify_restored_output_timings(&baseline,&[second,first]).is_ok());
        assert!(verify_restored_output_timings(&baseline,
            &[first,ActiveOutputTiming{mode:75_000,..second}]).is_err());
        assert!(verify_restored_output_timings(&baseline,
            &[first,ActiveOutputTiming{position:(0,1080),..second}]).is_err());
        assert!(verify_restored_output_timings(&baseline,&[first]).is_err());
        assert!(verify_restored_output_timings(&baseline,&[first,first]).is_err());
        let third=ActiveOutputTiming{crtc:control::from_u32(23).unwrap(),..second};
        assert!(verify_restored_output_timings(&baseline,&[first,third]).is_err());
        assert!(verify_restored_output_timings::<u32>(&[],&[]).is_err());
    }
    #[test]
    fn post_dispatch_vt_epoch_rejects_fb_timing_position_and_crtc_races() {
        let first=route(11,21,31,1920,1080);
        let second=route(12,22,32,1280,720);
        let routes=NativeOutputTopology{active:vec![first,second]};
        let timing_first=ActiveOutputTiming {
            crtc:first.crtc,mode:60_000u32,position:(0,0),
        };
        let timing_second=ActiveOutputTiming {
            crtc:second.crtc,mode:60_000u32,position:(1920,0),
        };
        let timings=[timing_first,timing_second];
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&timings).is_ok());
        let changed_fb=NativeOutputTopology{active:vec![
            ActiveOutputRoute{original_fb:control::from_u32(99).unwrap(),..first},
            second,
        ]};
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&changed_fb,&timings).is_err());
        let refresh=[timing_first,ActiveOutputTiming{mode:75_000,..timing_second}];
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&refresh).is_err());
        let shifted=[timing_first,ActiveOutputTiming{position:(1920,200),..timing_second}];
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&shifted).is_err());
        let duplicate=[timing_first,timing_first];
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&duplicate).is_err());
        let foreign=[timing_first,ActiveOutputTiming{
            crtc:control::from_u32(23).unwrap(),..timing_second
        }];
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&foreign).is_err());
        assert!(verify_unchanged_scanout_epoch(&routes,&timings,&routes,&timings[..1]).is_err());
    }
    #[test]
    fn active_crtc_readbacks_reject_same_size_mode_timing_position_and_fb_changes() {
        // The Mode field in production is the complete drm::control::Mode,
        // whose Eq includes timings. Here a simple numeric mode signature
        // isolates the readback comparison without a privileged DRM FD.
        let baseline=ActiveCrtcReadback {
            crtc:control::from_u32(21).unwrap(),
            framebuffer:control::from_u32(31).unwrap(),
            mode:60_000u32,
            position:(0,0),
        };
        assert_eq!([baseline],[baseline]);
        assert_ne!([baseline],[ActiveCrtcReadback{mode:75_000,..baseline}]);
        assert_ne!([baseline],[ActiveCrtcReadback{position:(1920,0),..baseline}]);
        assert_ne!([baseline],[ActiveCrtcReadback{
            framebuffer:control::from_u32(32).unwrap(),..baseline
        }]);
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
