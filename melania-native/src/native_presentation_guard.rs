//! Stage 89: epoch-bound presentation authorization for the isolated QEMU bridge.
//!
//! An ACK from an external DRM/KMS worker cannot grant wl_surface.frame or
//! wl_buffer.release after libseat loses the VT. The *same* generation must
//! be active at queue time and at CRTC acknowledgement time.
//!
//! Deliberately fail-closed: after a pause, a later ActivateSession does not
//! re-arm this guard. Re-arming requires a future, real re-acquisition and
//! revalidation of the DRM master, output configuration and framebuffer
//! lifetime. This is NOT production KMS ownership or a native login session.

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PresentationToken { epoch:u64 }

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PresentationPhase { Cold, Active, Revoked }

impl Default for PresentationPhase {
    fn default()->Self {Self::Cold}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PresentationError { SeatInactive, AlreadyStarted }

#[derive(Default,Debug)]
pub struct NativePresentationGuard {
    phase:PresentationPhase,
    epoch:u64,
}

impl NativePresentationGuard {
    /// Called exactly once on the initially active QEMU libseat generation.
    /// Merely receiving ActivateSession later must not bypass VT rollback.
    pub fn arm_initial(&mut self,seat_active:bool)->Result<(),PresentationError> {
        if self.phase!=PresentationPhase::Cold {
            return Err(PresentationError::AlreadyStarted);
        }
        if !seat_active {return Err(PresentationError::SeatInactive);}
        self.epoch=self.epoch.wrapping_add(1);
        self.phase=PresentationPhase::Active;
        Ok(())
    }

    pub fn phase(&self)->PresentationPhase {self.phase}

    /// A seat pause or libinput resume failure invalidates every outstanding
    /// worker job, whether or not the worker later reports an actual FB_ID.
    pub fn revoke(&mut self) {
        self.epoch=self.epoch.wrapping_add(1);
        self.phase=PresentationPhase::Revoked;
    }

    /// A token proves only that the caller observed an active seat when
    /// enqueueing the frame. It is NOT permission to acquire DRM master.
    pub fn ticket(&self,seat_active:bool)->Option<PresentationToken> {
        (seat_active && self.phase==PresentationPhase::Active)
            .then_some(PresentationToken{epoch:self.epoch})
    }

    /// The KMS presenter and callback path must both check the original
    /// ticket, even if libseat has since emitted ActivateSession.
    pub fn accepts(&self,token:PresentationToken,seat_active:bool)->bool {
        self.ticket(seat_active)==Some(token)
    }

    pub fn permanently_revoked(&self)->bool {
        self.phase==PresentationPhase::Revoked
    }

    /// This guard alone never authorizes an installable native login session.
    pub fn native_login_authorized(&self)->bool {false}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_seat_and_inactive_seat_never_present() {
        let mut guard=NativePresentationGuard::default();
        assert_eq!(guard.phase(),PresentationPhase::Cold);
        assert!(guard.ticket(true).is_none());
        assert_eq!(guard.arm_initial(false),Err(PresentationError::SeatInactive));
        assert!(guard.ticket(false).is_none());
        assert!(!guard.native_login_authorized());
    }

    #[test]
    fn in_flight_frame_is_valid_only_on_its_original_active_seat() {
        let mut guard=NativePresentationGuard::default();
        guard.arm_initial(true).unwrap();
        let token=guard.ticket(true).unwrap();
        assert!(guard.accepts(token,true));
        assert!(!guard.accepts(token,false));
        guard.revoke();
        assert_eq!(guard.phase(),PresentationPhase::Revoked);
        assert!(!guard.accepts(token,true));
        assert!(guard.ticket(true).is_none());
    }

    #[test]
    fn resume_without_full_drm_rollback_is_not_rearmed() {
        let mut guard=NativePresentationGuard::default();
        guard.arm_initial(true).unwrap();
        guard.revoke();
        assert_eq!(guard.arm_initial(true),Err(PresentationError::AlreadyStarted));
        assert!(guard.permanently_revoked());
        assert!(guard.ticket(true).is_none());
        assert!(!guard.native_login_authorized());
    }

    #[test]
    fn repeated_revocation_cannot_revalidate_queued_frames() {
        let mut guard=NativePresentationGuard::default();
        guard.arm_initial(true).unwrap();
        let old=guard.ticket(true).unwrap();
        guard.revoke();
        guard.revoke();
        assert!(!guard.accepts(old,true));
        assert!(guard.ticket(true).is_none());
    }
}
