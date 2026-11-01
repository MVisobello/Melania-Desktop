//! In-memory fencing for the future KMS render/page-flip loop.
//!
//! This gate does NOT call DRM, allocate scanout buffers or authorize KMS
//! presentation. It ensures that render candidates prepared before seat
//! suspension or GPU hotplug cannot be reused after resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateTicket {
    pub generation: u64,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateError {
    SeatInactive,
    NoVerifiedPrimaryPlane,
    CandidateAlreadyPending,
}

#[derive(Debug, Default)]
pub struct NativeFrameGate {
    seat_active: bool,
    generation: u64,
    next_sequence: u64,
    pending: Option<CandidateTicket>,
}

impl NativeFrameGate {
    pub fn seat_active(&self)->bool {self.seat_active}
    pub fn generation(&self)->u64 {self.generation}
    pub fn pending(&self)->Option<CandidateTicket> {self.pending}

    /// Invalidate stale GPU work on BOTH pause and resume, even if the new
    /// logical seat state is unchanged. The future renderer must drop old
    /// buffer handles, reprobe planes and rebuild its GBM/EGL resources.
    pub fn set_seat_active(&mut self, active:bool) {
        self.seat_active=active;
        self.invalidate();
    }

    /// Hotplug, device removal and connector reprobe invalidate pending work.
    pub fn invalidate(&mut self) {
        self.generation=self.generation.wrapping_add(1);
        self.pending=None;
    }

    /// This is ONLY a CPU-side render *candidate*, not a submitted KMS frame.
    /// Actual page flip is impossible in the present native implementation.
    pub fn prepare_candidate(
        &mut self, primary_plane_compatible:bool,
    )->Result<CandidateTicket,CandidateError> {
        if !self.seat_active {return Err(CandidateError::SeatInactive);}
        if !primary_plane_compatible {return Err(CandidateError::NoVerifiedPrimaryPlane);}
        if self.pending.is_some() {return Err(CandidateError::CandidateAlreadyPending);}
        self.next_sequence=self.next_sequence.wrapping_add(1);
        let ticket=CandidateTicket {
            generation:self.generation,sequence:self.next_sequence,
        };
        self.pending=Some(ticket);
        Ok(ticket)
    }

    /// Completing a stale candidate is always rejected, preventing a resumed
    /// compositor from treating an old GPU fence as a freshly rendered frame.
    pub fn retire_candidate(&mut self, ticket:CandidateTicket)->bool {
        if !self.seat_active || self.pending!=Some(ticket) {
            return false;
        }
        self.pending=None;
        true
    }

    pub fn actual_kms_submission_enabled(&self)->bool {false}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_before_seat_acquisition() {
        let mut gate=NativeFrameGate::default();
        assert_eq!(gate.prepare_candidate(true),Err(CandidateError::SeatInactive));
        assert!(!gate.actual_kms_submission_enabled());
    }
    #[test]
    fn rejects_missing_primary_plane_and_concurrent_candidates() {
        let mut gate=NativeFrameGate::default();
        gate.set_seat_active(true);
        assert_eq!(gate.prepare_candidate(false),Err(CandidateError::NoVerifiedPrimaryPlane));
        let ticket=gate.prepare_candidate(true).unwrap();
        assert_eq!(gate.prepare_candidate(true),Err(CandidateError::CandidateAlreadyPending));
        assert!(gate.retire_candidate(ticket));
        assert!(!gate.retire_candidate(ticket));
    }
    #[test]
    fn pause_resume_invalidates_old_gpu_fences() {
        let mut gate=NativeFrameGate::default();
        gate.set_seat_active(true);
        let before=gate.prepare_candidate(true).unwrap();
        gate.set_seat_active(false);
        assert_eq!(gate.pending(),None);
        assert!(!gate.retire_candidate(before));
        gate.set_seat_active(true);
        assert!(!gate.retire_candidate(before));
        let after=gate.prepare_candidate(true).unwrap();
        assert_ne!(before.generation,after.generation);
        assert!(gate.retire_candidate(after));
        assert!(!gate.actual_kms_submission_enabled());
    }
    #[test]
    fn gpu_hotplug_rejects_prior_candidate() {
        let mut gate=NativeFrameGate::default();
        gate.set_seat_active(true);
        let before=gate.prepare_candidate(true).unwrap();
        gate.invalidate();
        assert!(!gate.retire_candidate(before));
        let after=gate.prepare_candidate(true).unwrap();
        assert!(gate.retire_candidate(after));
    }
    #[test]
    fn repeated_activation_cannot_reuse_in_flight_work() {
        let mut gate=NativeFrameGate::default();
        gate.set_seat_active(true);
        let old=gate.prepare_candidate(true).unwrap();
        gate.set_seat_active(true);
        assert!(!gate.retire_candidate(old));
        assert!(gate.pending().is_none());
    }
}
