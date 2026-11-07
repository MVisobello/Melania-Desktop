//! Kernel DRM page-flip event routing for Melania's native outputs.
//!
//! A shared DRM card can report completion for DIFFERENT CRTCs in the same
//! receive_events() batch. Dropping the others is unsafe for a later
//! multi-monitor renderer. This bounded router drains the entire batch,
//! preserves events for other CRTCs, and rejects duplicate/misrouted events.
//!
//! KMS event delivery only proves that a flip completed on a CRTC. Callers
//! MUST additionally validate their per-output flip ledger and the live
//! framebuffer + connector -> encoder -> CRTC route before releasing any
//! Wayland frame callback or buffer.

use drm::control::{self, crtc, Device as ControlDevice};
use rustix::event::{poll, PollFd, PollFlags};
use std::{collections::VecDeque, os::fd::AsFd, time::{Duration, Instant}};

/// Each CRTC should have one outstanding KMS flip. A bounded queue protects
/// the display event loop against stale notifications and unbounded growth.
/// GPU hotplug, libseat pause or error requires a FRESH router/device epoch.
const MAX_PENDING_CRTC_EVENTS: usize = 16;

#[derive(Debug, Default)]
pub struct DrmFlipEventRouter {
    events: VecDeque<(crtc::Handle, u32)>,
    // Bound to the real active kernel CRTC topology for this libseat epoch.
    owned_crtcs: Option<Vec<crtc::Handle>>,
    poisoned: bool,
}

impl DrmFlipEventRouter {
    pub fn queued(&self) -> usize { self.events.len() }
    pub fn poisoned(&self) -> bool { self.poisoned }

    /// Bind this event router exactly once to the currently owned DRM CRTCs.
    /// A duplicate, changed, missing or over-capacity route fails closed.
    pub fn bind_active_crtcs(&mut self, crtcs: &[crtc::Handle]) -> Result<(), String> {
        if self.poisoned || self.owned_crtcs.is_some() || !self.events.is_empty() ||
           crtcs.is_empty() || crtcs.len()>crate::native_output_topology::MAX_ACTIVE_DRM_OUTPUTS ||
           crtcs.iter().enumerate().any(|(i,crtc)|crtcs[..i].contains(crtc)) {
            self.revoke();
            return Err("DRM page-flip router cannot bind invalid or stale CRTC routes".into());
        }
        self.owned_crtcs=Some(crtcs.to_vec());
        Ok(())
    }

    fn check_owned_crtc(&mut self, crtc: crtc::Handle) -> Result<(), String> {
        if self.owned_crtcs.as_ref().is_some_and(|owned|!owned.contains(&crtc)) {
            self.revoke();
            return Err("DRM page-flip event targets an unowned CRTC".into());
        }
        Ok(())
    }

    /// Permanently discard this device-epoch routing state. Reuse after VT
    /// pause, device replacement, or an ambiguous batch is forbidden.
    pub fn revoke(&mut self) {
        self.events.clear();
        self.poisoned = true;
    }

    /// Route one real kernel PageFlip event. Never silently drop a different
    /// CRTC's completion simply because another output is being serviced.
    /// More than one queued event for the same CRTC is ambiguous when
    /// page_flip has no per-request token exposed by drm-rs 0.14.
    pub fn enqueue(&mut self, crtc: crtc::Handle, frame: u32) -> Result<(), String> {
        if self.poisoned {
            return Err("DRM event router revoked".into());
        }
        self.check_owned_crtc(crtc)?;
        if self.events.len() >= MAX_PENDING_CRTC_EVENTS ||
           self.events.iter().any(|(pending, _)| *pending == crtc) {
            self.revoke();
            return Err("DRM event router ambiguous or at capacity".into());
        }
        self.events.push_back((crtc, frame));
        Ok(())
    }

    /// Consume only the expected output's completion; preserve every other
    /// output's pending event until its caller takes it.
    pub fn take(&mut self, expected: crtc::Handle) -> Result<Option<u32>, String> {
        if self.poisoned {
            return Err("DRM event router revoked".into());
        }
        self.check_owned_crtc(expected)?;
        let Some(index) = self.events.iter().position(|(crtc,_)| *crtc == expected) else {
            return Ok(None);
        };
        Ok(self.events.remove(index).map(|(_,frame)| frame))
    }
}

/// Wait on an owned DRM FD, routing *all* events in each read batch so a
/// future multi-CRTC caller does not discard another display's vblank event.
/// The router must be owned alongside the same DRM device+libseat epoch and
/// not shared across paused, unplugged or reopened devices.
pub fn wait_for_kms_page_flip_routed(
    card: &(impl ControlDevice + AsFd),
    expected_crtc: crtc::Handle,
    timeout: Duration,
    router: &mut DrmFlipEventRouter,
) -> Result<u32, String> {
    if timeout.is_zero() {
        return Err("DRM page flip timeout must be positive".into());
    }
    if let Some(frame) = router.take(expected_crtc)? {
        return Ok(frame);
    }
    let deadline=Instant::now().checked_add(timeout)
        .ok_or("DRM page flip timeout overflow")?;
    loop {
        let remaining=deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            router.revoke();
            return Err("DRM page flip vblank event timed out".into());
        }
        // rustix 0.38 takes a millisecond i32 deadline rather than Timespec.
        let timeout_ms=remaining.as_millis().clamp(1,i32::MAX as u128) as i32;
        let mut fds=[PollFd::new(card, PollFlags::IN)];
        let ready=match poll(&mut fds,timeout_ms) {
            Ok(ready)=>ready,
            Err(rustix::io::Errno::INTR)=>continue,
            Err(error)=>{
                router.revoke();
                return Err(format!("DRM event poll failed: {error}"));
            }
        };
        if ready==0 {
            router.revoke();
            return Err("DRM page flip vblank event timed out".into());
        }
        let revents=fds[0].revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            router.revoke();
            return Err("DRM event FD lost, hung up or became invalid".into());
        }
        if !revents.contains(PollFlags::IN) { continue; }
        let events=card.receive_events()
            .map_err(|error|{
                router.revoke();
                format!("DRM receive_events failed: {error}")
            })?;
        // Fully drain the read before returning. Returning early would lose
        // other CRTCs' events because the drm-rs iterator owns this batch.
        for event in events {
            if let control::Event::PageFlip(done)=event {
                router.enqueue(done.crtc, done.frame)?;
            }
        }
        if let Some(frame)=router.take(expected_crtc)? {
            return Ok(frame);
        }
    }
}

/// Legacy single-output compatibility, used only by isolated one-CRTC
/// probes. Multi-output DRM owners MUST retain one DrmFlipEventRouter across
/// all their output handlers and call wait_for_kms_page_flip_routed instead.
pub fn wait_for_kms_page_flip(
    card: &(impl ControlDevice + AsFd),
    expected_crtc: crtc::Handle,
    timeout: Duration,
) -> Result<u32, String> {
    let mut router=DrmFlipEventRouter::default();
    wait_for_kms_page_flip_routed(card, expected_crtc, timeout, &mut router)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crtc(id:u32)->crtc::Handle {
        control::from_u32(id).expect("nonzero typed DRM CRTC handle")
    }

    #[test]
    fn preserves_interleaved_kernel_events_from_other_monitors() {
        let mut events=DrmFlipEventRouter::default();
        let first=crtc(41);
        let second=crtc(42);
        let third=crtc(43);
        events.enqueue(second,100).unwrap();
        events.enqueue(first,101).unwrap();
        events.enqueue(third,102).unwrap();
        assert_eq!(events.queued(),3);
        assert_eq!(events.take(first).unwrap(),Some(101));
        assert_eq!(events.take(second).unwrap(),Some(100));
        assert_eq!(events.take(third).unwrap(),Some(102));
        assert_eq!(events.take(first).unwrap(),None);
    }

    #[test]
    fn duplicate_same_crtc_event_revokes_device_epoch() {
        let mut events=DrmFlipEventRouter::default();
        let monitor=crtc(51);
        events.enqueue(monitor,1).unwrap();
        assert!(events.enqueue(monitor,2).is_err());
        assert!(events.poisoned());
        assert_eq!(events.queued(),0);
        assert!(events.take(monitor).is_err());
        assert!(events.enqueue(monitor,3).is_err());
    }


    #[test]
    fn bound_router_preserves_two_owned_outputs_but_rejects_foreign_crtc() {
        let a=crtc(21); let b=crtc(22); let foreign=crtc(23);
        let mut routed=DrmFlipEventRouter::default();
        routed.bind_active_crtcs(&[a,b]).unwrap();
        routed.enqueue(b,101).unwrap();
        routed.enqueue(a,102).unwrap();
        assert_eq!(routed.take(a).unwrap(),Some(102));
        assert_eq!(routed.take(b).unwrap(),Some(101));
        routed.enqueue(a,103).unwrap();
        // A foreign/old-session CRTC can never release a Wayland ACK.
        assert!(routed.enqueue(foreign,104).is_err());
        assert!(routed.poisoned());
        assert_eq!(routed.queued(),0);
        assert!(routed.take(a).is_err());
    }

    #[test]
    fn bound_router_rejects_invalid_routes_and_foreign_waits() {
        let a=crtc(30); let other=crtc(31);
        let mut empty=DrmFlipEventRouter::default();
        assert!(empty.bind_active_crtcs(&[]).is_err());
        assert!(empty.poisoned());
        let mut duplicate=DrmFlipEventRouter::default();
        assert!(duplicate.bind_active_crtcs(&[a,a]).is_err());
        assert!(duplicate.poisoned());
        let mut bound=DrmFlipEventRouter::default();
        bound.bind_active_crtcs(&[a]).unwrap();
        assert!(bound.bind_active_crtcs(&[a]).is_err());
        assert!(bound.poisoned());
        let mut expected=DrmFlipEventRouter::default();
        expected.bind_active_crtcs(&[a]).unwrap();
        assert!(expected.take(other).is_err());
        assert!(expected.poisoned());
    }

    #[test]
    fn bounded_pending_outputs_and_independent_new_epoch() {
        let mut events=DrmFlipEventRouter::default();
        for id in 1..=MAX_PENDING_CRTC_EVENTS as u32 {
            events.enqueue(crtc(id),id).unwrap();
        }
        assert_eq!(events.queued(),MAX_PENDING_CRTC_EVENTS);
        assert!(events.enqueue(crtc(99),99).is_err());
        assert!(events.poisoned());
        let mut replacement=DrmFlipEventRouter::default();
        replacement.enqueue(crtc(99),1).unwrap();
        assert_eq!(replacement.take(crtc(99)).unwrap(),Some(1));
    }
}
