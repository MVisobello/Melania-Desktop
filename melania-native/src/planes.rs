//! DRM primary-plane capabilities, queried without creating surfaces or modesets.
//!
//! A usable CRTC/encoder path is not enough: the primary plane must also
//! accept a format+modifier the renderer can allocate. Even that match is
//! only a precondition, not permission to submit a page flip.
use smithay::backend::{
    allocator::{Fourcc, Modifier},
    drm::DrmDevice,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryPlaneSnapshot {
    pub crtc: String,
    pub primary_plane_count: usize,
    pub supported_format_count: usize,
    pub offscreen_abgr_linear_compatible: bool,
}

/// Read Smithay's already discovered primary planes per CRTC, without
/// claiming planes or modifying the KMS configuration.
pub fn inspect_primary_planes(device:&DrmDevice) -> Vec<PrimaryPlaneSnapshot> {
    let mut snapshots=Vec::new();
    for crtc in device.crtcs() {
        let mut snapshot=PrimaryPlaneSnapshot {
            crtc:format!("{crtc:?}"),
            primary_plane_count:0,
            supported_format_count:0,
            offscreen_abgr_linear_compatible:false,
        };
        match device.planes(crtc) {
            Ok(planes)=>{
                snapshot.primary_plane_count=planes.primary.len();
                for plane in &planes.primary {
                    snapshot.supported_format_count+=plane.formats.iter().count();
                    if plane.formats.iter().any(|format|
                        format.code==Fourcc::Abgr8888 && format.modifier==Modifier::Linear) {
                        snapshot.offscreen_abgr_linear_compatible=true;
                    }
                }
            }
            Err(error)=>eprintln!("Melania native primary planes for {crtc:?}: {error}"),
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|a,b|a.crtc.cmp(&b.crtc));
    snapshots
}

/// A unit-testable, read-only preflight: no primary plane or no exact
/// format/modifier match means the stage6 render buffer is not scanout-ready.
/// It deliberately never authorizes a KMS commit.
pub fn eligible_for_future_scanout(p:&PrimaryPlaneSnapshot)->bool {
    p.primary_plane_count>0 && p.offscreen_abgr_linear_compatible
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(count:usize,formats:usize,match_format:bool)->PrimaryPlaneSnapshot {
        PrimaryPlaneSnapshot {
            crtc:"crtc-0".to_string(),primary_plane_count:count,
            supported_format_count:formats,offscreen_abgr_linear_compatible:match_format,
        }
    }
    #[test]
    fn no_primary_plane_is_never_ready_even_when_formats_are_reported() {
        assert!(!eligible_for_future_scanout(&p(0,2,true)));
    }
    #[test]
    fn mismatched_modifier_is_not_a_valid_stage6_scanout_candidate() {
        assert!(!eligible_for_future_scanout(&p(1,4,false)));
    }
    #[test]
    fn format_match_is_only_a_candidate_not_an_actual_scanout() {
        let candidate=p(1,4,true);
        assert!(eligible_for_future_scanout(&candidate));
        assert!(!crate::renderer::native_scanout_is_implemented());
        assert!(!crate::NativeLifecycle::default().can_present_kms());
    }
}
