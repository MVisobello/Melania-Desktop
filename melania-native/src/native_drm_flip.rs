//! Kernel vblank acknowledgement for a real DRM page flip.
//!
//! This helper is used by Melania's isolated VirtIO KMS/GBM renderers.
//! The caller MUST already own the active libseat DRM descriptor and have
//! submitted PageFlipFlags::EVENT on this same card/CRTC. A completion event
//! alone is not proof of scanout: the caller also verifies framebuffer ID,
//! connector route, and presentation generation before notifying Wayland.
//!
//! Intentionally no hardcoded frame budget, sleep-based successful ACK or
//! implicit fallback to polling CRTC state when a kernel event is absent.

use drm::control::{self, crtc, Device as ControlDevice};
use rustix::event::{poll, PollFd, PollFlags};
use std::{os::fd::AsFd, time::{Duration, Instant}};

/// Wait for the *kernel* completion event associated with the only outstanding
/// page flip on this device/CRTC. This uses a deadline-bounded FD poll rather
/// than repeatedly polling the CRTC or busy-spinning. A wrong CRTC, VBlank,
/// signal/error or timeout cannot finish the frame.
pub fn wait_for_kms_page_flip(
    card: &(impl ControlDevice + AsFd),
    expected_crtc: crtc::Handle,
    timeout: Duration,
) -> Result<(), String> {
    if timeout.is_zero() {
        return Err("DRM page flip timeout must be positive".into());
    }
    let deadline=Instant::now().checked_add(timeout)
        .ok_or("DRM page flip timeout overflow")?;
    loop {
        let remaining=deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("DRM page flip vblank event timed out".into());
        }
        // rustix 0.38 accepts a millisecond i32 timeout (rather than
        // the timespec API in rustix 1.x). Round up short deadlines without
        // spinning, and keep all kernel waits strictly bounded.
        let timeout_ms=remaining.as_millis().clamp(1,i32::MAX as u128) as i32;
        let mut fds=[PollFd::new(card, PollFlags::IN)];
        let ready=match poll(&mut fds,timeout_ms) {
            Ok(ready)=>ready,
            Err(rustix::io::Errno::INTR)=>continue,
            Err(error)=>return Err(format!("DRM event poll failed: {error}")),
        };
        if ready==0 {
            return Err("DRM page flip vblank event timed out".into());
        }
        let revents=fds[0].revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            return Err("DRM event FD lost, hung up or became invalid".into());
        }
        if !revents.contains(PollFlags::IN) { continue; }
        for event in card.receive_events()
            .map_err(|error|format!("DRM receive_events failed: {error}"))? {
            if let control::Event::PageFlip(done)=event {
                if done.crtc==expected_crtc {
                    return Ok(());
                }
            }
        }
    }
}
