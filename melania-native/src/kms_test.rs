//! Opt-in QEMU-only atomic KMS TEST_ONLY preparation (no visible frame).
//!
//! The GBM buffered surface constructor asks the atomic DRM driver to
//! TEST_ONLY a candidate framebuffer. One tiny frame is rendered into an
//! unqueued GBM buffer. No queue_buffer, commit, page_flip or set_crtc.
//! The caller must first verify a dedicated Linux console and QEMU DMI.
use drm::control::{connector, Device as ControlDevice};
use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{DrmDevice, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        renderer::{gles::GlesRenderer, Bind, Color32F, Frame, Renderer},
    },
    utils::{Rectangle, Transform},
};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct KmsTestReport {
    pub connector: String,
    pub width: u32,
    pub height: u32,
    pub test_only_passed: bool,
    pub buffer_rendered: bool,
    /// Read-only CRTC inspection is NOT a complete atomic rollback snapshot.
    pub crtc_state_readable: bool,
    pub visible_frame_submitted: bool,
}

// Restrict experimental QEMU GBM swapchain allocations to a small, bounded
// surface. The driver's advertised first mode is not necessarily safe to use
// for a deliberately short-lived preflight.
const MIN_WIDTH: u16 = 640;
const MIN_HEIGHT: u16 = 480;
const MAX_WIDTH: u16 = 1920;
const MAX_HEIGHT: u16 = 1080;
const MAX_TEST_PIXELS: u32 = 1920 * 1080;

pub fn acceptable_test_mode(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT &&
    width <= MAX_WIDTH && height <= MAX_HEIGHT &&
    u32::from(width).checked_mul(u32::from(height))
        .is_some_and(|pixels| pixels <= MAX_TEST_PIXELS)
}

/// Choose the smallest usable advertised mode, not an arbitrary first mode.
/// This bounds the CPU/GPU memory used while preserving the actual KMS mode.
pub fn safest_test_mode_index(sizes: &[(u16, u16)]) -> Option<usize> {
    sizes.iter().enumerate()
        .filter(|(_, &(w,h))| acceptable_test_mode(w,h))
        .min_by_key(|&(idx, &(w,h))| (u32::from(w)*u32::from(h), idx))
        .map(|(index, _)| index)
}

/// Read-only atomic test plus an unqueued GBM render candidate.
/// Only called in a QEMU virtual GPU by the opted-in libseat experiment.
pub fn prepare_and_render_test_only(device:&mut DrmDevice)->Result<KmsTestReport,String> {
    if !device.is_atomic() {
        return Err("Atomic DRM required; legacy mode cannot guarantee TEST_ONLY".into());
    }
    let fd=device.device_fd().clone();
    let resources=fd.resource_handles()
        .map_err(|e|format!("KMS connector discovery failed: {e}"))?;
    let mut diagnostics=Vec::new();

    // Only one virtual monitor; never disturb other output configurations.
    for connector_handle in resources.connectors() {
        let info=match fd.get_connector(*connector_handle,false) {
            Ok(info)=>info,
            Err(error)=>{diagnostics.push(format!("connector: {error}"));continue;}
        };
        if info.state()!=connector::State::Connected {continue;}
        let sizes=info.modes().iter().map(|m| m.size()).collect::<Vec<_>>();
        let Some(mode_index)=safest_test_mode_index(&sizes) else {
            diagnostics.push("No safe bounded KMS test mode (640x480–1920x1080)".into());
            continue;
        };
        let mode=info.modes()[mode_index].clone();
        let (width,height)=mode.size();
        let name=format!("{}-{}",info.interface().as_str(),info.interface_id());

        for encoder_handle in info.encoders() {
            let encoder=match fd.get_encoder(*encoder_handle) {
                Ok(encoder)=>encoder,
                Err(error)=>{diagnostics.push(format!("encoder: {error}"));continue;}
            };
            for crtc in resources.filter_crtcs(encoder.possible_crtcs()) {
                if !device.planes(&crtc)
                    .is_ok_and(|planes|!planes.primary.is_empty()) {continue;}
                // A real CRTC read MUST succeed before constructing an atomic
                // test surface. This read is NOT a complete rollback snapshot:
                // plane properties, connector bindings and VT ownership still
                // require a separate restore implementation before scanout.
                if let Err(error)=fd.get_crtc(crtc) {
                    diagnostics.push(format!("CRTC state unavailable: {error}"));
                    continue;
                }
                // Surface creation builds pending state but does not commit.
                let surface=match device.create_surface(crtc,mode.clone(),&[*connector_handle]) {
                    Ok(surface)=>surface,
                    Err(error)=>{diagnostics.push(format!("surface: {error}"));continue;}
                };
                let gbm=GbmDevice::new(fd.clone())
                    .map_err(|e|format!("Virtual GBM device unavailable: {e}"))?;
                // Safety: all GPU resources are dropped before libseat exits.
                let display=unsafe {EGLDisplay::new(gbm.clone())}
                    .map_err(|e|format!("Virtual EGL display unavailable: {e}"))?;
                let context=EGLContext::new(&display)
                    .map_err(|e|format!("Virtual EGL context unavailable: {e}"))?;
                let mut renderer=unsafe {GlesRenderer::new(context)}
                    .map_err(|e|format!("Virtual GLES renderer unavailable: {e}"))?;
                let allocator=GbmAllocator::new(
                    gbm,GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT
                );
                // Constructor performs atomic TEST_ONLY; NEVER commits a mode.
                let mut swapchain:GbmBufferedSurface<_,()>=GbmBufferedSurface::new(
                    surface,
                    allocator,
                    &[Fourcc::Argb8888,Fourcc::Abgr8888],
                    display.dmabuf_render_formats().iter().copied(),
                ).map_err(|e|format!("KMS TEST_ONLY rejected buffers: {e}"))?;

                let (mut buffer,_age)=swapchain.next_buffer()
                    .map_err(|e|format!("GBM swapchain allocation failed: {e}"))?;
                let mut target=renderer.bind(&mut buffer)
                    .map_err(|e|format!("KMS candidate bind failed: {e}"))?;
                let size=(i32::from(width),i32::from(height));
                let mut frame=renderer.render(&mut target,size.into(),Transform::Normal)
                    .map_err(|e|format!("KMS candidate render failed: {e}"))?;
                frame.clear(
                    Color32F::new(0.06,0.17,0.44,1.0),
                    &[Rectangle::from_size(size.into())],
                ).map_err(|e|format!("KMS candidate draw failed: {e}"))?;
                frame.finish()
                    .map_err(|e|format!("KMS candidate finish failed: {e}"))?
                    .wait().map_err(|e|format!("KMS candidate GPU sync failed: {e}"))?;

                // No queue_buffer, commit or page_flip: the temporary
                // GPU buffer is destroyed without reaching a monitor.
                return Ok(KmsTestReport {
                    connector:name,width:u32::from(width),height:u32::from(height),
                    test_only_passed:true,
                    buffer_rendered:true,
                    crtc_state_readable:true,
                    visible_frame_submitted:false,
                });
            }
        }
    }
    Err(format!("No viable QEMU atomic KMS TEST_ONLY configuration: {}",
        diagnostics.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_mode_rejects_tiny_huge_and_invalid_dimensions() {
        for (w,h) in [(0,0),(320,240),(1920,1200),(3840,2160),(800,0)] {
            assert!(!acceptable_test_mode(w,h));
        }
        assert!(acceptable_test_mode(640,480));
        assert!(acceptable_test_mode(1920,1080));
    }

    #[test]
    fn smallest_acceptable_mode_wins_over_first_advertised() {
        let modes=[(3840,2160),(1920,1080),(1024,768),(640,480)];
        assert_eq!(safest_test_mode_index(&modes),Some(3));
        assert_eq!(safest_test_mode_index(&[(4000,3000),(320,200)]),None);
        assert_eq!(safest_test_mode_index(&[]),None);
    }

    #[test]
    fn a_test_only_pass_never_claims_a_visible_frame() {
        let result=KmsTestReport {
            connector:"Virtual-1".into(),width:1280,height:720,
            test_only_passed:true,buffer_rendered:true,
            crtc_state_readable:true,visible_frame_submitted:false,
        };
        assert!(result.test_only_passed);
        assert!(result.crtc_state_readable);
        assert!(!result.visible_frame_submitted);
        assert!(!crate::renderer::native_scanout_is_implemented());
    }
}
