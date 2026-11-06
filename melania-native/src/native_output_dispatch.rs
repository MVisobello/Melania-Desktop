//! Stage 94: one coordinated, fail-closed output dispatch ledger.
//!
//! This owns NO DRM card, GBM surface, framebuffer, libseat session or VT.
//! A future production backend must feed actual libseat/udev transitions and
//! kernel-confirmed CRTC scanout into this coordinator. Passing a boolean
//! here does NOT establish hardware ownership or authorize native login.
//!
//! Unlike the finite QEMU receiver, this ledger has no frame-count budget.
//! At most one frame may be in flight. A lost VT, output change, bad flip
//! or missed deadline invalidates all outstanding permission to release
//! Wayland buffers or send client frame callbacks.

use drm::control::{connector, crtc, framebuffer, Device as ControlDevice};

use crate::{
    continuous_redraw::{ContinuousRedraw, PlannedSceneFrame},
    kms_watchdog::{KmsFrameWatchdog, WatchdogError},
    native_resource_recovery::{
        NativeResourceRecovery, RecoveryEpoch, RecoveryError, RecoveryPhase,
        ResourceMilestone, ScanoutPermit,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSubmission {
    pub scene: PlannedSceneFrame,
    pub resource: ScanoutPermit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputDispatchError {
    PolicyUnavailable,
    Watchdog(WatchdogError),
}

#[derive(Debug, Default)]
pub struct NativeOutputDispatch {
    resources: NativeResourceRecovery,
    redraw: ContinuousRedraw,
    watchdog: KmsFrameWatchdog,
    inflight: Option<OutputSubmission>,
}

impl NativeOutputDispatch {
    pub fn phase(&self) -> RecoveryPhase { self.resources.phase() }
    pub fn inflight(&self) -> Option<OutputSubmission> { self.inflight }
    pub fn pending_generation(&self) -> Option<u64> {
        self.redraw.pending().map(|f| f.generation)
    }
    pub fn presented_generation(&self) -> Option<u64> {
        self.redraw.presented_generation()
    }
    pub fn scene_generation(&self) -> Option<u64> {
        self.redraw.observed_generation()
    }

    pub fn begin_acquire(&mut self) -> Result<RecoveryEpoch, RecoveryError> {
        self.resources.begin_acquire()
    }

    pub fn confirm(
        &mut self, epoch: RecoveryEpoch, step: ResourceMilestone,
    ) -> Result<(), RecoveryError> {
        self.resources.confirm(epoch, step)
    }

    /// Activation requires ALL eight real resource proofs from the caller.
    /// The coordinator cannot independently assert that those proofs are real.
    pub fn activate(&mut self, epoch: RecoveryEpoch) -> Result<(), RecoveryError> {
        self.resources.activate(epoch)?;
        self.inflight = None;
        self.watchdog.reset(0);
        self.redraw.set_seat_active(true);
        Ok(())
    }

    /// Record the latest retained scene generation even during VT pause,
    /// device reacquisition or first startup. Such notifications are NOT
    /// permission to submit a frame: the inactive scheduler will reject
    /// damage, while remembering the newest generation for the mandatory
    /// full repaint after fresh resource proofs and activation.
    ///
    /// Without this, a client commit while the seat was suspended would be
    /// silently discarded by the dispatch ledger; on resume the old scene
    /// could be replayed until another client happened to redraw.
    pub fn scene_changed(&mut self, generation: u64) -> bool {
        let queued = self.redraw.scene_changed(generation);
        self.resources.phase() == RecoveryPhase::Active && queued
    }

    /// No blocking DRM calls take place here. The caller must composite and
    /// submit the scene only after a Some() grant, then report the REAL
    /// kernel CRTC result before any Wayland frame or release callbacks.
    pub fn dispatch(
        &mut self, now_ms: u64, timeout_ms: u64,
    ) -> Result<Option<OutputSubmission>, OutputDispatchError> {
        if self.resources.phase() != RecoveryPhase::Active {
            return Ok(None);
        }
        let Some(scene) = self.redraw.next_frame() else {
            return Ok(None);
        };
        let Some(resource) = self.resources.next_frame() else {
            self.fail_closed();
            return Err(OutputDispatchError::PolicyUnavailable);
        };
        if self.watchdog.epoch() != scene.ticket.epoch {
            self.watchdog.reset(scene.ticket.epoch);
        }
        if let Err(error) = self.watchdog.arm(scene.ticket, now_ms, timeout_ms) {
            self.fail_closed();
            return Err(OutputDispatchError::Watchdog(error));
        }
        let submission = OutputSubmission { scene, resource };
        self.inflight = Some(submission);
        Ok(Some(submission))
    }

    /// Verify a presentation against REAL DRM kernel state, including the
    /// connector -> encoder -> CRTC route, rather than trusting a socket ACK
    /// or allowing an external boolean to release Wayland client buffers.
    ///
    /// The caller must own this device through an active libseat session and
    /// must not call after a pause/hotplug. The caller also owns the lifetime
    /// and scanout compatibility of the expected framebuffer.
    ///
    /// Stale submissions are discarded without probing or cancelling the
    /// current frame. Missing routing or FB_ID permanently invalidates the
    /// pending output generation.
    pub fn verify_and_present(
        &mut self,
        card: &impl ControlDevice,
        connector: connector::Handle,
        crtc: crtc::Handle,
        expected_fb: framebuffer::Handle,
        submission: OutputSubmission,
        now_ms: u64,
    ) -> Result<bool, String> {
        if self.resources.phase() != RecoveryPhase::Active ||
            self.inflight != Some(submission) {
            return Ok(false);
        }
        // Use the exact kernel probe also exercised by the real QEMU KMS
        // receiver, not a mockable or caller-provided success flag.
        let verified = crate::native_kms_scanout::verify_kernel_scanout(
            card, crtc, connector, expected_fb,
        );
        match verified {
            Ok(kernel_presented) => Ok(self.presented(
                submission, now_ms, kernel_presented,
            )),
            Err(error) => {
                self.fail_closed();
                Err(error)
            }
        }
    }

    /// Internal-only commit path. External backends cannot claim a
    /// successful presentation from an unverified boolean.
    fn presented(
        &mut self, submission: OutputSubmission,
        now_ms: u64, kernel_presented: bool,
    ) -> bool {
        if self.inflight != Some(submission) ||
            self.resources.phase() != RecoveryPhase::Active {
            return false;
        }
        if !kernel_presented ||
            !self.watchdog.acknowledged(submission.scene.ticket, now_ms) ||
            !self.resources.acknowledge_frame(submission.resource) ||
            !self.redraw.presented(submission.scene) {
            self.fail_closed();
            return false;
        }
        self.inflight = None;
        true
    }

    /// A stale failure must never cancel a newer epoch's presentation.
    /// A matching failed flip poisons this resource generation; no retry is
    /// safe until devices have been released and freshly reacquired.
    pub fn rejected(&mut self, submission: OutputSubmission) -> bool {
        if self.inflight != Some(submission) { return false; }
        self.fail_closed();
        true
    }

    /// Call from the real monotonic timer source in the future DRM loop.
    pub fn expired(&mut self, now_ms: u64) -> bool {
        let Some(submission) = self.inflight else { return false; };
        if self.watchdog.expired(now_ms) != Some(submission.scene.ticket) {
            return false;
        }
        self.fail_closed();
        true
    }

    /// Revoke tickets BEFORE the outer backend pauses libinput, DRM or VT.
    pub fn pause(&mut self) -> Result<(), RecoveryError> {
        self.resources.pause()?;
        self.inflight = None;
        self.watchdog.reset(0);
        self.redraw.set_seat_active(false);
        Ok(())
    }

    pub fn begin_restore(&mut self) -> Result<RecoveryEpoch, RecoveryError> {
        self.resources.begin_restore()
    }

    /// GPU unplug, modeset uncertainty, fatal I/O, or incomplete rollback.
    pub fn fail_closed(&mut self) {
        self.resources.fail();
        self.inflight = None;
        self.watchdog.reset(0);
        self.redraw.set_seat_active(false);
    }

    pub fn native_login_authorized(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROOFS: [ResourceMilestone; 8] = [
        ResourceMilestone::ActiveSeat,
        ResourceMilestone::OwnedDrmMaster,
        ResourceMilestone::GbmRenderer,
        ResourceMilestone::OwnedOriginalScanout,
        ResourceMilestone::ConfiguredOutput,
        ResourceMilestone::RestorableVt,
        ResourceMilestone::EnumeratedInput,
        ResourceMilestone::VerifiedRollback,
    ];

    fn prove_all(loop_state: &mut NativeOutputDispatch, epoch: RecoveryEpoch) {
        for step in PROOFS { loop_state.confirm(epoch, step).unwrap(); }
        loop_state.activate(epoch).unwrap();
    }

    fn active() -> NativeOutputDispatch {
        let mut s = NativeOutputDispatch::default();
        let e = s.begin_acquire().unwrap();
        prove_all(&mut s, e);
        s
    }

    #[test]
    fn no_scanout_until_all_eight_resources_revalidated() {
        let mut s = NativeOutputDispatch::default();
        assert!(!s.scene_changed(1));
        assert_eq!(s.dispatch(100, 250), Ok(None));
        let epoch = s.begin_acquire().unwrap();
        s.confirm(epoch, ResourceMilestone::ActiveSeat).unwrap();
        assert_eq!(s.activate(epoch), Err(RecoveryError::IncompleteResources));
        assert_eq!(s.dispatch(100, 250), Ok(None));
        assert!(!s.native_login_authorized());
    }

    #[test]
    fn hundreds_of_confirmed_generations_without_qemu_frame_limit() {
        let mut s = active();
        for generation in 1..=256 {
            assert!(s.scene_changed(generation));
            let frame = s.dispatch(generation * 20, 250).unwrap().unwrap();
            assert_eq!(frame.scene.generation, generation);
            assert_eq!(s.dispatch(generation * 20, 250), Ok(None));
            assert!(s.presented(frame, generation * 20 + 1, true));
            assert!(!s.presented(frame, generation * 20 + 2, true));
        }
        assert_eq!(s.presented_generation(), Some(256));
        assert_eq!(s.dispatch(9000, 250), Ok(None));
    }

    #[test]
    fn three_updates_during_flip_become_one_successor() {
        let mut s = active();
        s.scene_changed(1);
        let first = s.dispatch(10, 100).unwrap().unwrap();
        for n in 2..=9 { s.scene_changed(n); }
        assert_eq!(s.dispatch(11, 100), Ok(None));
        assert!(s.presented(first, 12, true));
        let next = s.dispatch(13, 100).unwrap().unwrap();
        assert_eq!(next.scene.generation, 9);
        assert_ne!(first.resource, next.resource);
        assert!(s.presented(next, 14, true));
    }

    #[test]
    fn pause_invalidates_queued_ack_and_resume_requires_full_proof() {
        let mut s = active();
        s.scene_changed(4);
        let old = s.dispatch(100, 200).unwrap().unwrap();
        s.pause().unwrap();
        assert!(!s.presented(old, 101, true));
        assert_eq!(s.dispatch(102, 200), Ok(None));
        let epoch = s.begin_restore().unwrap();
        assert_eq!(s.activate(epoch), Err(RecoveryError::IncompleteResources));
        prove_all(&mut s, epoch);
        assert!(!s.presented(old, 103, true));
        s.scene_changed(5);
        let new = s.dispatch(104, 200).unwrap().unwrap();
        assert_ne!(new.resource.epoch, old.resource.epoch);
        assert!(!s.presented(old, 105, true));
        assert!(s.presented(new, 106, true));
    }

    #[test]
    fn vt_paused_scene_updates_replay_newest_only_after_reacquisition() {
        let mut s = active();
        assert!(s.scene_changed(40));
        let old = s.dispatch(10, 100).unwrap().unwrap();
        s.pause().unwrap();
        // A client may commit twice while the VT is away. These changes
        // are retained but are NEVER legal scanout or client callbacks.
        assert!(!s.scene_changed(41));
        assert!(!s.scene_changed(42));
        assert_eq!(s.scene_generation(), Some(42));
        assert_eq!(s.dispatch(20, 100), Ok(None));
        assert_eq!(s.presented_generation(), None);
        let epoch = s.begin_restore().unwrap();
        assert_eq!(s.activate(epoch), Err(RecoveryError::IncompleteResources));
        assert_eq!(s.dispatch(21, 100), Ok(None));
        prove_all(&mut s, epoch);
        let fresh = s.dispatch(30, 100).unwrap().unwrap();
        assert_eq!(fresh.scene.generation, 42);
        assert_ne!(fresh.scene.ticket, old.scene.ticket);
        assert_ne!(fresh.resource.epoch, old.resource.epoch);
        assert!(!s.presented(old, 31, true));
        assert_eq!(s.inflight(), Some(fresh));
        assert!(s.presented(fresh, 32, true));
        assert_eq!(s.presented_generation(), Some(42));
        assert_eq!(s.dispatch(33, 100), Ok(None));
    }

    #[test]
    fn first_startup_carries_preexisting_scene_to_first_verified_frame() {
        let mut s = NativeOutputDispatch::default();
        assert!(!s.scene_changed(7));
        assert!(!s.scene_changed(8));
        assert_eq!(s.scene_generation(), Some(8));
        assert_eq!(s.dispatch(1, 100), Ok(None));
        let epoch = s.begin_acquire().unwrap();
        assert!(!s.scene_changed(9));
        assert_eq!(s.dispatch(2, 100), Ok(None));
        prove_all(&mut s, epoch);
        let first = s.dispatch(3, 100).unwrap().unwrap();
        assert_eq!(first.scene.generation, 9);
        assert!(s.presented(first, 4, true));
        assert_eq!(s.dispatch(5, 100), Ok(None));
    }

    #[test]
    fn stale_undeliverable_commits_do_not_rearm_failed_resources() {
        let mut s = active();
        assert!(s.scene_changed(1));
        let frame = s.dispatch(1, 50).unwrap().unwrap();
        assert!(s.rejected(frame));
        assert!(!s.scene_changed(2));
        assert_eq!(s.scene_generation(), Some(2));
        assert_eq!(s.dispatch(10, 50), Ok(None));
        assert_eq!(s.phase(), RecoveryPhase::Failed);
        assert!(!s.native_login_authorized());
    }

    #[test]
    fn stale_ack_does_not_cancel_new_pending_frame() {
        let mut s = active();
        s.scene_changed(1);
        let first = s.dispatch(1, 100).unwrap().unwrap();
        assert!(s.presented(first, 2, true));
        s.scene_changed(2);
        let second = s.dispatch(3, 100).unwrap().unwrap();
        assert!(!s.presented(first, 4, true));
        assert_eq!(s.inflight(), Some(second));
        assert!(s.presented(second, 5, true));
    }

    #[test]
    fn timeouts_and_unverified_ack_poison_generation() {
        let mut s = active();
        s.scene_changed(2);
        let frame = s.dispatch(100, 50).unwrap().unwrap();
        assert!(!s.expired(149));
        assert!(!s.presented(frame, 150, true));
        assert_eq!(s.phase(), RecoveryPhase::Failed);
        assert_eq!(s.dispatch(151, 250), Ok(None));

        let mut s = active();
        s.scene_changed(3);
        let frame = s.dispatch(100, 50).unwrap().unwrap();
        assert!(!s.presented(frame, 101, false));
        assert_eq!(s.phase(), RecoveryPhase::Failed);
    }

    #[test]
    fn watchdog_error_and_hotplug_fail_closed() {
        let mut s = active();
        s.scene_changed(1);
        assert_eq!(
            s.dispatch(2, 0),
            Err(OutputDispatchError::Watchdog(WatchdogError::InvalidTimeout))
        );
        assert_eq!(s.phase(), RecoveryPhase::Failed);
        assert!(!s.native_login_authorized());

        let mut s = active();
        s.scene_changed(1);
        let stale = s.dispatch(3, 100).unwrap().unwrap();
        s.fail_closed();
        assert!(!s.presented(stale, 4, true));
        assert!(!s.rejected(stale));
        assert_eq!(s.phase(), RecoveryPhase::Failed);
    }

    #[test]
    fn explicit_rejection_halts_stream_without_frame_callback() {
        let mut s = active();
        s.scene_changed(4);
        let frame = s.dispatch(100, 200).unwrap().unwrap();
        assert!(s.rejected(frame));
        assert!(!s.presented(frame, 101, true));
        assert_eq!(s.presented_generation(), None);
        assert_eq!(s.phase(), RecoveryPhase::Failed);
    }
}
