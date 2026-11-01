//! Stage 13: fail-closed ownership and lifecycle guard for QEMU TEST_ONLY.
//!
//! This is an in-process guard for *observed* KMS state, not a KMS restore
//! manager. It does not preserve kernel-owned framebuffer objects, cannot
//! recover a VT and can never authorize a visible frame. An external timeout
//! remains necessary if a GPU driver blocks inside an ioctl.
use std::time::{Duration, Instant};
use crate::{kms_restore::AtomicBaseline, kms_resources::KmsResourceInventory};

const MAX_PREFLIGHT: Duration = Duration::from_secs(40);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightPhase {
    Captured,
    Verified,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightError {
    SeatInactive,
    Expired,
    Invalidated,
    BaselineChanged,
    ResourcesChanged,
    InvalidBudget,
}

pub struct NativePreflightGuard {
    before: AtomicBaseline,
    resources: KmsResourceInventory,
    started: Instant,
    budget: Duration,
    phase: PreflightPhase,
}

impl NativePreflightGuard {
    /// Move the baseline and blob bytes into the guard; callers cannot
    /// accidentally drop only one half of the captured preflight state.
    pub fn capture(
        before: AtomicBaseline,
        resources: KmsResourceInventory,
        started: Instant,
        budget: Duration,
    ) -> Result<Self, PreflightError> {
        if budget.is_zero() || budget > MAX_PREFLIGHT {
            return Err(PreflightError::InvalidBudget);
        }
        // Only a completely readable atomic inventory is acceptable.
        if before.validate().is_err() {
            return Err(PreflightError::Invalidated);
        }
        Ok(Self {
            before, resources, started, budget,
            phase: PreflightPhase::Captured,
        })
    }

    pub fn phase(&self) -> PreflightPhase { self.phase }
    pub fn copied_blobs(&self) -> usize { self.resources.copied_blob_count() }
    pub fn observed_framebuffers(&self) -> usize { self.resources.borrowed_framebuffer_count() }

    /// Must be called around each blocking GPU stage. This is a *cooperative*
    /// deadline, not a replacement for the external watchdog that can kill
    /// a stalled ioctl. Libseat callbacks still need a proper event loop
    /// before a native graphical session could be safely enabled.
    pub fn checkpoint(
        &mut self,
        seat_active: bool,
        now: Instant,
    ) -> Result<(), PreflightError> {
        if self.phase != PreflightPhase::Captured {
            return Err(PreflightError::Invalidated);
        }
        if !seat_active {
            self.phase = PreflightPhase::Aborted;
            return Err(PreflightError::SeatInactive);
        }
        let elapsed = now.checked_duration_since(self.started);
        if elapsed.is_none_or(|elapsed| elapsed > self.budget) {
            self.phase = PreflightPhase::Aborted;
            return Err(PreflightError::Expired);
        }
        Ok(())
    }

    /// Pause, hotplug and device removal should invalidate a pending test.
    /// Once invalidated, a guard can NEVER be reused after VT activation.
    pub fn invalidate(&mut self) {
        self.phase = PreflightPhase::Aborted;
    }

    /// Compare property IDs, object bindings AND the copied blob payloads.
    /// A successful comparison confirms only TEST_ONLY was non-destructive;
    /// it does not establish a working rollback or native scanout.
    pub fn verify_after(
        &mut self,
        after: &AtomicBaseline,
        after_resources: &KmsResourceInventory,
        seat_active: bool,
        now: Instant,
    ) -> Result<(), PreflightError> {
        self.checkpoint(seat_active, now)?;
        if !after.unchanged_since(&self.before) {
            self.invalidate();
            return Err(PreflightError::BaselineChanged);
        }
        if !after_resources.unchanged_since(&self.resources) {
            self.invalidate();
            return Err(PreflightError::ResourcesChanged);
        }
        self.phase = PreflightPhase::Verified;
        Ok(())
    }

    pub fn restoration_implemented(&self) -> bool { false }
    pub fn visible_frame_allowed(&self) -> bool { false }
}

impl Drop for NativePreflightGuard {
    fn drop(&mut self) {
        // Best-effort clearing of temporary userspace blob copies when the
        // guard goes out of scope. This does NOT release kernel framebuffer
        // ownership because those objects were never acquired.
        for blob in &mut self.resources.blobs {
            blob.bytes.fill(0);
        }
        self.phase = PreflightPhase::Aborted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kms_restore::{AtomicObject, AtomicProperty};
    use crate::kms_resources::capture_with;

    fn object(kind: &'static str, handle: &str, pairs: &[(&str,u64)]) -> AtomicObject {
        AtomicObject {
            kind, handle: handle.into(),
            properties: pairs.iter().enumerate().map(|(id,(name,value))| AtomicProperty {
                handle: format!("property-{id}"), name:(*name).into(),value:*value,
            }).collect(),
        }
    }
    fn setup(now: Instant) -> (NativePreflightGuard, AtomicBaseline, KmsResourceInventory) {
        let baseline=AtomicBaseline { objects: vec![
            object("crtc","crtc0",&[("ACTIVE",1),("MODE_ID",42)]),
            object("connector","connector0",&[("CRTC_ID",1)]),
            object("plane","plane0",&[
                ("CRTC_ID",1),("FB_ID",17),("SRC_X",0),("SRC_Y",0),
                ("SRC_W",640),("SRC_H",480),("CRTC_X",0),("CRTC_Y",0),
                ("CRTC_W",640),("CRTC_H",480),
            ]),
        ]};
        let resources=capture_with(&baseline,|_|Ok(vec![1,2,3])).unwrap();
        let guard=NativePreflightGuard::capture(
            baseline.clone(),resources.clone(),now,Duration::from_secs(5)).unwrap();
        (guard,baseline,resources)
    }
    #[test]
    fn successful_read_only_comparison_never_authorizes_scanout() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        assert_eq!(guard.phase(),PreflightPhase::Captured);
        assert_eq!(guard.copied_blobs(),1);
        assert_eq!(guard.observed_framebuffers(),1);
        assert!(guard.checkpoint(true,now).is_ok());
        assert!(guard.verify_after(&baseline,&resources,true,
            now+Duration::from_secs(3)).is_ok());
        assert_eq!(guard.phase(),PreflightPhase::Verified);
        assert!(!guard.visible_frame_allowed());
        assert!(!guard.restoration_implemented());
    }
    #[test]
    fn pause_invalidates_every_pending_guard_even_after_resume() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        assert_eq!(guard.checkpoint(false,now),Err(PreflightError::SeatInactive));
        assert_eq!(guard.phase(),PreflightPhase::Aborted);
        assert_eq!(guard.verify_after(&baseline,&resources,true,now),
            Err(PreflightError::Invalidated));
    }
    #[test]
    fn gpu_hotplug_cannot_reuse_a_stale_snapshot() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        guard.invalidate();
        assert_eq!(guard.verify_after(&baseline,&resources,true,now),
            Err(PreflightError::Invalidated));
    }
    #[test]
    fn bounded_deadline_fails_closed_and_stays_aborted() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        assert_eq!(guard.checkpoint(true,now+Duration::from_secs(6)),
            Err(PreflightError::Expired));
        assert_eq!(guard.verify_after(&baseline,&resources,true,now),
            Err(PreflightError::Invalidated));
        assert!(matches!(NativePreflightGuard::capture(
            baseline,resources,now,Duration::from_secs(60)),
            Err(PreflightError::InvalidBudget)));
    }
    #[test]
    fn detects_changed_atomic_properties_and_blob_payloads() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        let mut changed=baseline.clone();
        changed.objects[0].properties[0].value=0;
        assert_eq!(guard.verify_after(&changed,&resources,true,now),
            Err(PreflightError::BaselineChanged));
        let (mut guard,baseline,resources)=setup(now);
        let mut altered=resources.clone();
        altered.blobs[0].bytes[0]^=1;
        assert_eq!(guard.verify_after(&baseline,&altered,true,now),
            Err(PreflightError::ResourcesChanged));
    }
    #[test]
    fn refuses_invalid_clock_or_zero_budget() {
        let now=Instant::now();
        let (mut guard,baseline,resources)=setup(now);
        assert_eq!(guard.checkpoint(true,now-Duration::from_millis(1)),
            Err(PreflightError::Expired));
        assert!(matches!(NativePreflightGuard::capture(
            baseline,resources,now,Duration::ZERO),
            Err(PreflightError::InvalidBudget)));
    }
}
