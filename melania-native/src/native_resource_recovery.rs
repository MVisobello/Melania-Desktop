//! Stage 92: fail-closed resource/VT recovery contract for an eventual
//! long-running DRM/GBM compositor.
//!
//! This is a **policy** object, not a hardware driver. The caller must provide
//! real evidence for each milestone after acquiring devices via libseat;
//! neither setting a bit nor passing these tests proves a working DRM master,
//! GBM scanout, owned original framebuffer, or restored VT.
//!
//! The QEMU bridge cannot use this object to enable native login. A real
//! backend must bind the milestones to checked kernel/libseat operations
//! and treat every lost device, failed flip or pause as a full invalidation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryPhase {
    Cold,
    Acquiring,
    Active,
    Suspended,
    Restoring,
    Failed,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceMilestone {
    ActiveSeat = 0,
    OwnedDrmMaster = 1,
    GbmRenderer = 2,
    OwnedOriginalScanout = 3,
    ConfiguredOutput = 4,
    RestorableVt = 5,
    EnumeratedInput = 6,
    VerifiedRollback = 7,
}

const ALL_MILESTONES: u16 = (1 << 8) - 1;

impl ResourceMilestone {
    fn bit(self) -> u16 { 1u16 << (self as u8) }

    fn prerequisites(self) -> u16 {
        match self {
            ResourceMilestone::ActiveSeat => 0,
            ResourceMilestone::OwnedDrmMaster |
            ResourceMilestone::EnumeratedInput |
            ResourceMilestone::RestorableVt => ResourceMilestone::ActiveSeat.bit(),
            ResourceMilestone::GbmRenderer |
            ResourceMilestone::OwnedOriginalScanout => ResourceMilestone::OwnedDrmMaster.bit(),
            ResourceMilestone::ConfiguredOutput =>
                ResourceMilestone::GbmRenderer.bit() |
                ResourceMilestone::OwnedOriginalScanout.bit(),
            ResourceMilestone::VerifiedRollback =>
                ResourceMilestone::ConfiguredOutput.bit() |
                ResourceMilestone::RestorableVt.bit(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryEpoch(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanoutPermit {
    pub epoch: u64,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    InvalidPhase,
    StaleEpoch,
    PrerequisiteMissing,
    IncompleteResources,
    CounterExhausted,
}

#[derive(Debug)]
pub struct NativeResourceRecovery {
    phase: RecoveryPhase,
    epoch: u64,
    sequence: u64,
    milestones: u16,
    inflight: Option<ScanoutPermit>,
}

impl Default for NativeResourceRecovery {
    fn default() -> Self {
        Self {
            phase: RecoveryPhase::Cold,
            epoch: 0,
            sequence: 0,
            milestones: 0,
            inflight: None,
        }
    }
}

impl NativeResourceRecovery {
    pub fn phase(&self) -> RecoveryPhase { self.phase }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn pending(&self) -> Option<ScanoutPermit> { self.inflight }

    fn invalidate(&mut self) -> Result<RecoveryEpoch, RecoveryError> {
        self.inflight = None;
        self.milestones = 0;
        match self.epoch.checked_add(1) {
            Some(next) => {
                self.epoch = next;
                Ok(RecoveryEpoch(next))
            }
            None => {
                self.phase = RecoveryPhase::Failed;
                Err(RecoveryError::CounterExhausted)
            }
        }
    }

    /// Start acquisition once. It is not permission to manipulate DRM.
    pub fn begin_acquire(&mut self) -> Result<RecoveryEpoch, RecoveryError> {
        if self.phase != RecoveryPhase::Cold {
            return Err(RecoveryError::InvalidPhase);
        }
        let epoch = self.invalidate()?;
        self.phase = RecoveryPhase::Acquiring;
        Ok(epoch)
    }

    /// Record evidence only for the **current** acquisition/recovery epoch.
    /// Drivers must not call this until their hardware operation succeeded.
    pub fn confirm(
        &mut self, epoch: RecoveryEpoch, step: ResourceMilestone,
    ) -> Result<(), RecoveryError> {
        if !matches!(self.phase, RecoveryPhase::Acquiring | RecoveryPhase::Restoring) {
            return Err(RecoveryError::InvalidPhase);
        }
        if epoch.0 != self.epoch {
            return Err(RecoveryError::StaleEpoch);
        }
        if self.milestones & step.prerequisites() != step.prerequisites() {
            return Err(RecoveryError::PrerequisiteMissing);
        }
        self.milestones |= step.bit();
        Ok(())
    }

    pub fn activate(&mut self, epoch: RecoveryEpoch) -> Result<(), RecoveryError> {
        if !matches!(self.phase, RecoveryPhase::Acquiring | RecoveryPhase::Restoring) {
            return Err(RecoveryError::InvalidPhase);
        }
        if epoch.0 != self.epoch { return Err(RecoveryError::StaleEpoch); }
        if self.milestones != ALL_MILESTONES {
            return Err(RecoveryError::IncompleteResources);
        }
        self.phase = RecoveryPhase::Active;
        Ok(())
    }

    /// Revoke pending KMS ACKs **before** suspending libinput or releasing DRM.
    pub fn pause(&mut self) -> Result<(), RecoveryError> {
        if self.phase == RecoveryPhase::Failed {
            return Err(RecoveryError::InvalidPhase);
        }
        self.invalidate()?;
        self.phase = RecoveryPhase::Suspended;
        Ok(())
    }

    /// Receiving ActivateSession from libseat is not enough to resume.
    /// Every resource proof must be rebuilt for this new epoch.
    pub fn begin_restore(&mut self) -> Result<RecoveryEpoch, RecoveryError> {
        if self.phase != RecoveryPhase::Suspended {
            return Err(RecoveryError::InvalidPhase);
        }
        let epoch = self.invalidate()?;
        self.phase = RecoveryPhase::Restoring;
        Ok(epoch)
    }

    /// Lost GPU, failed KMS flip, abandoned restoration or process shutdown.
    /// A Failed object cannot be rearmed. A fresh session is required.
    pub fn fail(&mut self) {
        self.inflight = None;
        self.milestones = 0;
        self.epoch = self.epoch.saturating_add(1);
        self.phase = RecoveryPhase::Failed;
    }

    /// Only one in-flight frame may be submitted. No frame-count ceiling.
    /// This grant is still only a policy grant, never proof of DRM capability.
    pub fn next_frame(&mut self) -> Option<ScanoutPermit> {
        if self.phase != RecoveryPhase::Active || self.inflight.is_some() {
            return None;
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.fail();
            return None;
        };
        self.sequence = sequence;
        let permit = ScanoutPermit { epoch: self.epoch, sequence };
        self.inflight = Some(permit);
        Some(permit)
    }

    /// The real KMS backend must verify scanout on the actual CRTC first.
    /// A stale or duplicate ACK cannot retire a different generation.
    pub fn acknowledge_frame(&mut self, permit: ScanoutPermit) -> bool {
        if self.phase != RecoveryPhase::Active || self.inflight != Some(permit) {
            return false;
        }
        self.inflight = None;
        true
    }

    /// A lost VT, stale CRTC baseline or failed flip has no safe retry here.
    pub fn failed_frame(&mut self, permit: ScanoutPermit) -> bool {
        if self.phase != RecoveryPhase::Active || self.inflight != Some(permit) {
            return false;
        }
        self.fail();
        true
    }

    /// An installable login session additionally requires a real backend and
    /// dedicated hardware test, neither of which this policy module supplies.
    pub fn native_login_authorized(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [ResourceMilestone; 8] = [
        ResourceMilestone::ActiveSeat,
        ResourceMilestone::OwnedDrmMaster,
        ResourceMilestone::GbmRenderer,
        ResourceMilestone::OwnedOriginalScanout,
        ResourceMilestone::ConfiguredOutput,
        ResourceMilestone::RestorableVt,
        ResourceMilestone::EnumeratedInput,
        ResourceMilestone::VerifiedRollback,
    ];

    fn prove_all(recovery: &mut NativeResourceRecovery, epoch: RecoveryEpoch) {
        for step in ORDER { recovery.confirm(epoch, step).unwrap(); }
        recovery.activate(epoch).unwrap();
    }

    #[test]
    fn no_implicit_start_or_partially_recovered_scanout() {
        let mut state = NativeResourceRecovery::default();
        assert_eq!(state.next_frame(), None);
        assert!(!state.native_login_authorized());
        let epoch = state.begin_acquire().unwrap();
        assert_eq!(
            state.confirm(epoch, ResourceMilestone::ConfiguredOutput),
            Err(RecoveryError::PrerequisiteMissing)
        );
        state.confirm(epoch, ResourceMilestone::ActiveSeat).unwrap();
        assert_eq!(state.activate(epoch), Err(RecoveryError::IncompleteResources));
        assert!(state.next_frame().is_none());
    }

    #[test]
    fn repeated_frames_are_unbounded_but_never_concurrent() {
        let mut state = NativeResourceRecovery::default();
        let epoch = state.begin_acquire().unwrap();
        prove_all(&mut state, epoch);
        for sequence in 1..=512 {
            let ticket = state.next_frame().unwrap();
            assert_eq!(ticket.sequence, sequence);
            assert!(state.next_frame().is_none());
            assert!(state.acknowledge_frame(ticket));
            assert!(!state.acknowledge_frame(ticket));
        }
        assert!(!state.native_login_authorized());
    }

    #[test]
    fn pause_invalidates_pending_ack_and_requires_full_restoration() {
        let mut state = NativeResourceRecovery::default();
        let first_epoch = state.begin_acquire().unwrap();
        prove_all(&mut state, first_epoch);
        let old_frame = state.next_frame().unwrap();
        state.pause().unwrap();
        assert_eq!(state.phase(), RecoveryPhase::Suspended);
        assert!(!state.acknowledge_frame(old_frame));
        assert!(state.next_frame().is_none());
        let fresh_epoch = state.begin_restore().unwrap();
        assert_ne!(first_epoch, fresh_epoch);
        assert_eq!(
            state.confirm(first_epoch, ResourceMilestone::ActiveSeat),
            Err(RecoveryError::StaleEpoch)
        );
        assert_eq!(state.activate(fresh_epoch), Err(RecoveryError::IncompleteResources));
        prove_all(&mut state, fresh_epoch);
        assert!(!state.acknowledge_frame(old_frame));
        let new_frame = state.next_frame().unwrap();
        assert_ne!(old_frame.epoch, new_frame.epoch);
        assert!(state.acknowledge_frame(new_frame));
    }

    #[test]
    fn a_failed_flip_permanently_poisoned_this_session() {
        let mut state = NativeResourceRecovery::default();
        let epoch = state.begin_acquire().unwrap();
        prove_all(&mut state, epoch);
        let ticket = state.next_frame().unwrap();
        let fake = ScanoutPermit { sequence: ticket.sequence + 1, ..ticket };
        assert!(!state.failed_frame(fake));
        assert!(state.failed_frame(ticket));
        assert_eq!(state.phase(), RecoveryPhase::Failed);
        assert!(!state.acknowledge_frame(ticket));
        assert!(state.next_frame().is_none());
        assert_eq!(state.begin_restore(), Err(RecoveryError::InvalidPhase));
        assert_eq!(state.begin_acquire(), Err(RecoveryError::InvalidPhase));
    }

    #[test]
    fn input_and_vt_proofs_cannot_precede_seat_and_output() {
        let mut state = NativeResourceRecovery::default();
        let epoch = state.begin_acquire().unwrap();
        for step in [
            ResourceMilestone::OwnedDrmMaster,
            ResourceMilestone::EnumeratedInput,
            ResourceMilestone::RestorableVt,
            ResourceMilestone::VerifiedRollback,
        ] {
            assert_eq!(state.confirm(epoch, step), Err(RecoveryError::PrerequisiteMissing));
        }
        state.confirm(epoch, ResourceMilestone::ActiveSeat).unwrap();
        state.confirm(epoch, ResourceMilestone::RestorableVt).unwrap();
        assert_eq!(
            state.confirm(epoch, ResourceMilestone::VerifiedRollback),
            Err(RecoveryError::PrerequisiteMissing)
        );
    }

    #[test]
    fn repeated_pause_does_not_resurrect_old_permissions() {
        let mut state = NativeResourceRecovery::default();
        let epoch = state.begin_acquire().unwrap();
        prove_all(&mut state, epoch);
        let old = state.next_frame().unwrap();
        state.pause().unwrap();
        state.pause().unwrap();
        let restart = state.begin_restore().unwrap();
        prove_all(&mut state, restart);
        assert!(!state.acknowledge_frame(old));
        let fresh = state.next_frame().unwrap();
        assert!(state.acknowledge_frame(fresh));
    }
}
