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
    // Flip ioctls that actually succeeded but have not yet been acknowledged.
    // Only an armed CRTC may generate an acceptable kernel completion.
    armed_crtcs: Vec<crtc::Handle>,
    poisoned: bool,
}

impl DrmFlipEventRouter {
    pub fn queued(&self) -> usize { self.events.len() }
    /// A malformed wait must not leave an outstanding completion usable in
    /// this DRM acquisition epoch. Validate before consuming queued events.
    fn wait_deadline(&mut self, timeout: Duration) -> Result<Instant, String> {
        if self.poisoned {
            return Err("DRM event router revoked".into());
        }
        if timeout.is_zero() {
            self.revoke();
            return Err("DRM page flip timeout must be positive".into());
        }
        match Instant::now().checked_add(timeout) {
            Some(deadline) => Ok(deadline),
            None => {
                self.revoke();
                Err("DRM page flip timeout overflow".into())
            }
        }
    }
    pub fn poisoned(&self) -> bool { self.poisoned }

    /// Bind this event router exactly once to the currently owned DRM CRTCs.
    /// A duplicate, changed, missing or over-capacity route fails closed.
    pub fn bind_active_crtcs(&mut self, crtcs: &[crtc::Handle]) -> Result<(), String> {
        if self.poisoned || self.owned_crtcs.is_some() || !self.events.is_empty() ||
           !self.armed_crtcs.is_empty() ||
           crtcs.is_empty() || crtcs.len()>crate::native_output_topology::MAX_ACTIVE_DRM_OUTPUTS ||
           crtcs.iter().enumerate().any(|(i,crtc)|crtcs[..i].contains(crtc)) {
            self.revoke();
            return Err("DRM page-flip router cannot bind invalid or stale CRTC routes".into());
        }
        self.owned_crtcs=Some(crtcs.to_vec());
        Ok(())
    }

    fn check_owned_crtc(&mut self, crtc: crtc::Handle) -> Result<(), String> {
        if !self.owned_crtcs.as_ref().is_some_and(|owned|owned.contains(&crtc)) {
            self.revoke();
            return Err("DRM page-flip event targets an unowned CRTC".into());
        }
        Ok(())
    }

    /// Mark a successful page_flip(EVENT) ioctl on this owned CRTC. Call this
    /// immediately after the ioctl succeeds and BEFORE reading any DRM events.
    /// A second submission before consuming the first event is unsafe.
    pub fn arm_flip(&mut self, crtc: crtc::Handle) -> Result<(), String> {
        if self.poisoned {
            return Err("DRM event router revoked".into());
        }
        self.check_owned_crtc(crtc)?;
        if self.armed_crtcs.contains(&crtc) ||
           self.events.iter().any(|(pending, _)| *pending == crtc) {
            self.revoke();
            return Err("DRM CRTC already has an unacknowledged page flip".into());
        }
        self.armed_crtcs.push(crtc);
        Ok(())
    }

    /// Permanently discard this device-epoch routing state. Reuse after VT
    /// pause, device replacement, or an ambiguous batch is forbidden.
    pub fn revoke(&mut self) {
        self.events.clear();
        self.armed_crtcs.clear();
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
        // A page flip event without a successful request in this acquisition
        // epoch is stale or foreign: it must never satisfy the next frame.
        if !self.armed_crtcs.contains(&crtc) {
            self.revoke();
            return Err("DRM unsolicited page-flip completion".into());
        }
        // DRM PageFlip events do not promise an incrementing vblank counter:
        // VirtIO and some virtual devices reuse sequence numbers. The router
        // validates the actual armed ioctl, CRTC and bounded event batch.
        // The per-output NativeFlipLedger performs strict monotonic validation
        // on physical DRM, with a separately QEMU-gated compatibility path.
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
        if !self.armed_crtcs.contains(&expected) {
            self.revoke();
            return Err("DRM page-flip wait without a submitted request".into());
        }
        let Some(index) = self.events.iter().position(|(crtc,_)| *crtc == expected) else {
            return Ok(None);
        };
        // Consumption retires the specific CRTC's in-flight ioctl. Another
        // monitor's armed event remains available until separately consumed.
        self.armed_crtcs.retain(|crtc| *crtc != expected);
        Ok(self.events.remove(index).map(|(_,frame)| frame))
    }
}

/// A bounded poll interval is not the full deadline for a long wait.
/// This helper also keeps the decision deterministic in regression tests.
fn deadline_expired(now: Instant, deadline: Instant) -> bool {
    now >= deadline
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
    let deadline=router.wait_deadline(timeout)?;
    if let Some(frame) = router.take(expected_crtc)? {
        return Ok(frame);
    }
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
            // poll() is bounded to i32::MAX milliseconds, whereas the
            // caller's deadline may be much farther away. A completed
            // poll interval is not necessarily the end of the wait.
            // Recheck the monotonic deadline before revoking this epoch.
            if !deadline_expired(Instant::now(), deadline) {
                continue;
            }
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
    router.bind_active_crtcs(&[expected_crtc])?;
    // Legacy caller already submitted its single EVENT ioctl.
    router.arm_flip(expected_crtc)?;
    wait_for_kms_page_flip_routed(card, expected_crtc, timeout, &mut router)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crtc(id:u32)->crtc::Handle {
        control::from_u32(id).expect("nonzero typed DRM CRTC handle")
    }

    #[test]
    fn bounded_poll_interval_does_not_end_longer_deadline() {
        let start=Instant::now();
        let deadline=start.checked_add(Duration::from_secs(60 * 60 * 24 * 30)).unwrap();
        let first_interval=start.checked_add(Duration::from_millis(i32::MAX as u64)).unwrap();
        assert!(!deadline_expired(first_interval, deadline));
        assert!(deadline_expired(deadline, deadline));
        assert!(deadline_expired(deadline + Duration::from_millis(1), deadline));
    }

    #[test]
    fn invalid_wait_cannot_release_stale_crtc_completion() {
        let monitor=crtc(77);
        let mut router=DrmFlipEventRouter::default();
        router.bind_active_crtcs(&[monitor]).unwrap();
        router.arm_flip(monitor).unwrap();
        router.enqueue(monitor,123).unwrap();
        assert!(router.wait_deadline(Duration::ZERO).is_err());
        assert!(router.poisoned());
        assert_eq!(router.queued(),0);
        assert!(router.take(monitor).is_err());
        assert!(router.wait_deadline(Duration::from_millis(1)).is_err());
    }

    #[test]
    fn valid_wait_does_not_discard_other_crtc_events() {
        let a=crtc(78); let b=crtc(79);
        let mut router=DrmFlipEventRouter::default();
        router.bind_active_crtcs(&[a,b]).unwrap();
        router.arm_flip(a).unwrap();
        router.arm_flip(b).unwrap();
        router.enqueue(b,5).unwrap();
        assert!(router.wait_deadline(Duration::from_millis(50)).is_ok());
        assert_eq!(router.take(a).unwrap(),None);
        assert_eq!(router.take(b).unwrap(),Some(5));
    }

    #[test]
    fn unbound_router_revokes_on_kernel_event_or_wait() {
        let mut incoming=DrmFlipEventRouter::default();
        assert!(incoming.enqueue(crtc(17), 1).is_err());
        assert!(incoming.poisoned());
        let mut waiting=DrmFlipEventRouter::default();
        assert!(waiting.take(crtc(17)).is_err());
        assert!(waiting.poisoned());
    }

    #[test]
    fn preserves_interleaved_kernel_events_from_other_monitors() {
        let mut events=DrmFlipEventRouter::default();
        let first=crtc(41);
        let second=crtc(42);
        let third=crtc(43);
        events.bind_active_crtcs(&[first,second,third]).unwrap();
        for crtc in [first,second,third] { events.arm_flip(crtc).unwrap(); }
        events.enqueue(second,100).unwrap();
        events.enqueue(first,101).unwrap();
        events.enqueue(third,102).unwrap();
        assert_eq!(events.queued(),3);
        assert_eq!(events.take(first).unwrap(),Some(101));
        assert_eq!(events.take(second).unwrap(),Some(100));
        assert_eq!(events.take(third).unwrap(),Some(102));
        // A retired request cannot accept a replay or release a second ACK.
        assert!(events.take(first).is_err());
        assert!(events.poisoned());
    }

    #[test]
    fn duplicate_same_crtc_event_revokes_device_epoch() {
        let mut events=DrmFlipEventRouter::default();
        let monitor=crtc(51);
        events.bind_active_crtcs(&[monitor]).unwrap();
        events.arm_flip(monitor).unwrap();
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
        routed.arm_flip(a).unwrap();
        routed.arm_flip(b).unwrap();
        routed.enqueue(b,101).unwrap();
        routed.enqueue(a,102).unwrap();
        assert_eq!(routed.take(a).unwrap(),Some(102));
        assert_eq!(routed.take(b).unwrap(),Some(101));
        routed.arm_flip(a).unwrap();
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
    fn unsolicited_completion_revokes_all_armed_outputs() {
        let a=crtc(70); let b=crtc(71);
        let mut router=DrmFlipEventRouter::default();
        router.bind_active_crtcs(&[a,b]).unwrap();
        router.arm_flip(a).unwrap();
        // B has no successful page_flip ioctl in this device epoch.
        assert!(router.enqueue(b,200).is_err());
        assert!(router.poisoned());
        assert_eq!(router.queued(),0);
        assert!(router.take(a).is_err());
    }

    #[test]
    fn duplicate_submission_and_replayed_completion_cannot_ack_new_frame() {
        let a=crtc(80);
        let mut router=DrmFlipEventRouter::default();
        router.bind_active_crtcs(&[a]).unwrap();
        router.arm_flip(a).unwrap();
        assert!(router.arm_flip(a).is_err());
        assert!(router.poisoned());

        let mut fresh=DrmFlipEventRouter::default();
        fresh.bind_active_crtcs(&[a]).unwrap();
        fresh.arm_flip(a).unwrap();
        fresh.enqueue(a,1).unwrap();
        assert_eq!(fresh.take(a).unwrap(),Some(1));
        assert!(fresh.enqueue(a,1).is_err());
        assert!(fresh.poisoned());
    }

    #[test]
    fn qemu_unreliable_vblank_counter_does_not_poison_real_event_routing() {
        use crate::native_flip_ledger::NativeFlipLedger;
        let output=crtc(90);
        let mut router=DrmFlipEventRouter::default();
        let mut vm_ledger=NativeFlipLedger::default();
        router.bind_active_crtcs(&[output]).unwrap();
        // Two distinct successful kernel flip ioctls may report the SAME
        // vblank frame under QEMU VirtIO. Each must have its own real event.
        for frame in [42,42,41,41] {
            vm_ledger.begin_flip().unwrap();
            router.arm_flip(output).unwrap();
            router.enqueue(output,frame).unwrap();
            let receipt=router.take(output).unwrap().unwrap();
            vm_ledger.complete_vm_flip(receipt).unwrap();
        }
        assert_eq!(vm_ledger.completed_count(),4);
        assert!(vm_ledger.unreliable_vm_counter());
        assert!(!router.poisoned());
        // An unsolicited extra event remains fatal, even on virtual GPU.
        assert!(router.enqueue(output,41).is_err());
        assert!(router.poisoned());
    }

    #[test]
    fn physical_kernel_vblank_replay_remains_rejected_by_strict_ledger() {
        use crate::native_flip_ledger::{FlipLedgerError,NativeFlipLedger};
        let output=crtc(91);
        let mut router=DrmFlipEventRouter::default();
        let mut strict=NativeFlipLedger::default();
        router.bind_active_crtcs(&[output]).unwrap();
        for frame in [100,100] {
            strict.begin_flip().unwrap();
            router.arm_flip(output).unwrap();
            router.enqueue(output,frame).unwrap();
            let receipt=router.take(output).unwrap().unwrap();
            let result=strict.complete_flip(receipt);
            if frame==100 && strict.completed_count()==1 {
                assert!(result.is_ok());
            } else {
                assert_eq!(result,Err(FlipLedgerError::StaleOrDuplicateVblank));
            }
        }
        assert!(strict.revoked());
    }

    #[test]
    fn independent_crtcs_handle_vm_wrapping_without_losing_events() {
        use crate::native_flip_ledger::NativeFlipLedger;
        let a=crtc(92); let b=crtc(93);
        let mut router=DrmFlipEventRouter::default();
        let mut ledgers=[NativeFlipLedger::default(),NativeFlipLedger::default()];
        router.bind_active_crtcs(&[a,b]).unwrap();
        for frame in [u32::MAX,0,0] {
            for (index,output) in [a,b].into_iter().enumerate() {
                ledgers[index].begin_flip().unwrap();
                router.arm_flip(output).unwrap();
            }
            router.enqueue(b,frame).unwrap();
            router.enqueue(a,frame).unwrap();
            for (index,output) in [a,b].into_iter().enumerate() {
                let receipt=router.take(output).unwrap().unwrap();
                ledgers[index].complete_vm_flip(receipt).unwrap();
            }
        }
        assert_eq!(ledgers[0].completed_count(),3);
        assert_eq!(ledgers[1].completed_count(),3);
        assert!(!router.poisoned());
    }

    #[test]
    fn bounded_pending_outputs_and_independent_new_epoch() {
        let mut events=DrmFlipEventRouter::default();
        let active: Vec<_>=(1..=crate::native_output_topology::MAX_ACTIVE_DRM_OUTPUTS as u32).map(crtc).collect();
        events.bind_active_crtcs(&active).unwrap();
        for (index, output) in active.iter().enumerate() {
            events.arm_flip(*output).unwrap();
            events.enqueue(*output,index as u32 + 1).unwrap();
        }
        assert_eq!(events.queued(),active.len());
        assert!(events.enqueue(active[0],99).is_err());
        assert!(events.poisoned());
        let mut replacement=DrmFlipEventRouter::default();
        replacement.bind_active_crtcs(&[crtc(99)]).unwrap();
        replacement.arm_flip(crtc(99)).unwrap();
        replacement.enqueue(crtc(99),1).unwrap();
        assert_eq!(replacement.take(crtc(99)).unwrap(),Some(1));
    }
}
