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
            origin:(0,0),
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
}
