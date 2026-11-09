//! Fail-closed, per-connector DRM/GBM preflight planner.
//! A single successful offscreen render must not certify every attached
//! monitor. We allocate temporary owned buffers at EVERY active CRTC mode;
//! no KMS modesets, page flips, or login permissions are issued here.
use drm::control::{connector,crtc};
use crate::{native_output_topology::{NativeOutputTopology,validate_active_routes},
            renderer::validate_gbm_output_dimensions};

pub const MAX_TOTAL_PROBE_BYTES:usize=192*1024*1024;
const PROBE_BUFFERS:u32=2;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct OutputBufferProbe {
    pub connector:connector::Handle,
    pub crtc:crtc::Handle,
    pub width:u32,
    pub height:u32,
    pub bytes:usize,
}

/// Use the currently-attached *kernel CRTC mode*, not an unrelated output's
/// advertised EDID recommendation or a guessed default resolution.
/// The total reservation is capped across all outputs, not just per monitor.
pub fn plan_active_output_probes(
    topology:&NativeOutputTopology,
)->Result<Vec<OutputBufferProbe>,String> {
    validate_active_routes(&topology.active)?;
    let mut total=0usize;
    let mut probes=Vec::with_capacity(topology.active.len());
    for route in &topology.active {
        let width=u32::from(route.width);
        let height=u32::from(route.height);
        let bytes=validate_gbm_output_dimensions(width,height,PROBE_BUFFERS)?;
        total=total.checked_add(bytes)
            .ok_or("native multi-output GBM allocation overflow")?;
        if total>MAX_TOTAL_PROBE_BYTES {
            return Err("native multi-output GBM test exceeds aggregate 192 MiB budget".into());
        }
        probes.push(OutputBufferProbe{
            connector:route.connector,crtc:route.crtc,width,height,bytes,
        });
    }
    Ok(probes)
}


/// Verify the complete set of previously active output routes after a real
/// libseat VT pause and re-acquisition. A new FB_ID is legitimate when the
/// other VT's compositor has replaced its own scanout framebuffer; however,
/// losing a connector, moving its encoder/CRTC, or silently changing its
/// buffer dimensions must not be reported as restored physical output.
///
/// This is a read-only safety gate for the dedicated-VT hardware probe, not
/// hotplug recovery, KMS ownership or a mode-setting implementation.
pub fn verify_vt_output_restore(
    before:&NativeOutputTopology,
    after:&NativeOutputTopology,
)->Result<(),String> {
    validate_active_routes(&before.active)?;
    validate_active_routes(&after.active)?;
    if before.active.len()!=after.active.len() {
        return Err("VT resume changed the number of active DRM outputs".into());
    }
    for original in &before.active {
        let restored=after.active.iter()
            .find(|route|route.connector==original.connector)
            .ok_or("VT resume lost an original active DRM connector")?;
        if restored.original_encoder!=original.original_encoder ||
           restored.crtc!=original.crtc {
            return Err("VT resume changed an original connector/encoder/CRTC route".into());
        }
        if restored.width!=original.width || restored.height!=original.height {
            return Err("VT resume changed the original active DRM output dimensions".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use drm::control::{self,framebuffer};
    use crate::native_output_topology::ActiveOutputRoute;
    fn output(conn:u32,scanout:u32,fb:u32,w:u16,h:u16)->ActiveOutputRoute {
        ActiveOutputRoute {
            connector:control::from_u32(conn).unwrap(),
            crtc:control::from_u32(scanout).unwrap(),
            original_encoder:control::from_u32(conn+1000).unwrap(),
            original_fb:control::from_u32::<framebuffer::Handle>(fb).unwrap(),
            width:w,height:h,
        }
    }
    #[test]
    fn plans_every_physical_crtc_not_only_first_monitor() {
        let topo=NativeOutputTopology{active:vec![
            output(11,21,31,1920,1080),
            output(12,22,32,1280,720),
        ]};
        let plan=plan_active_output_probes(&topo).unwrap();
        assert_eq!(plan.len(),2);
        assert_eq!((plan[0].width,plan[0].height),(1920,1080));
        assert_eq!((plan[1].width,plan[1].height),(1280,720));
        assert_eq!(plan[0].bytes,1920*1080*4*2);
        assert_eq!(plan[1].bytes,1280*720*4*2);
        assert_ne!(plan[0].crtc,plan[1].crtc);
    }
    #[test]
    fn rejects_multiple_big_monitors_even_when_each_fits_own_budget() {
        let topo=NativeOutputTopology{active:(0..4).map(|i|
            output(i+1,i+11,i+21,3840,2160)
        ).collect()};
        assert!(plan_active_output_probes(&topo).is_err());
    }
    #[test]
    fn probe_preflight_rejects_two_distinct_crtcs_with_one_encoder() {
        // The inventory planner must honor the exact kernel encoder
        // ownership invariant even when connectors and CRTCs differ.
        let a=output(11,21,31,1920,1080);
        let mut b=output(12,22,32,1280,720);
        b.original_encoder=a.original_encoder;
        let topo=NativeOutputTopology{active:vec![a,b]};
        assert!(plan_active_output_probes(&topo).is_err());
    }
    #[test]
    fn missing_duplicate_or_oversized_active_routes_fail_closed() {
        let empty=NativeOutputTopology{active:vec![]};
        assert!(plan_active_output_probes(&empty).is_err());
        let duplicate=NativeOutputTopology{active:vec![
            output(1,11,21,1280,800),
            output(2,11,22,1280,800),
        ]};
        assert!(plan_active_output_probes(&duplicate).is_err());
        let too_wide=NativeOutputTopology{active:vec![output(1,11,21,5000,800)]};
        assert!(plan_active_output_probes(&too_wide).is_err());
    }

    #[test]
    fn physical_vt_resume_requires_every_original_connector_and_route() {
        let a=output(11,21,31,1920,1080);
        let b=output(12,22,32,1280,720);
        let baseline=NativeOutputTopology{active:vec![a,b]};
        // An external DRM master may change FB_ID across a real VT switch:
        // that is not proof of a missing connector or failed buffer reinit.
        let replaced_fb=ActiveOutputRoute {
            original_fb:control::from_u32::<framebuffer::Handle>(99).unwrap(),..a
        };
        let reordered=NativeOutputTopology{active:vec![b,replaced_fb]};
        assert!(verify_vt_output_restore(&baseline,&reordered).is_ok());
        assert!(verify_vt_output_restore(
            &baseline,&NativeOutputTopology{active:vec![a]}
        ).is_err());
        assert!(verify_vt_output_restore(
            &baseline,&NativeOutputTopology{active:vec![a,output(13,23,33,1280,720)]}
        ).is_err());
        assert!(verify_vt_output_restore(
            &baseline,&NativeOutputTopology{active:vec![a,ActiveOutputRoute{
                crtc:control::from_u32(23).unwrap(),..b
            }]}
        ).is_err());
        assert!(verify_vt_output_restore(
            &baseline,&NativeOutputTopology{active:vec![a,ActiveOutputRoute{
                original_encoder:control::from_u32(2012).unwrap(),..b
            }]}
        ).is_err());
        assert!(verify_vt_output_restore(
            &baseline,&NativeOutputTopology{active:vec![a,ActiveOutputRoute{
                width:1600,..b
            }]}
        ).is_err());
    }

}
