//! VT lifecycle policy; actual ioctls/session FD ownership are NOT wired.
//! Every pause, resume or abort revokes old presentation permits.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum VtPhase {Detached,Active,Suspending,Suspended,Restoring}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct VtFramePermit {epoch:u64}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum VtTransitionError {InvalidTransition,EpochExhausted}

#[derive(Debug)]
pub struct VtHandoff {phase:VtPhase,epoch:u64}
impl Default for VtHandoff {
    fn default()->Self {Self {phase:VtPhase::Detached,epoch:0}}
}
impl VtHandoff {
    pub fn phase(&self)->VtPhase {self.phase}
    pub fn epoch(&self)->u64 {self.epoch}
    fn transition(&mut self,expected:VtPhase,next:VtPhase)->Result<(),VtTransitionError> {
        if self.epoch==u64::MAX {
            // Even an already-active exhausted session must revoke permits.
            self.phase=VtPhase::Detached;
            return Err(VtTransitionError::EpochExhausted);
        }
        if self.phase!=expected {return Err(VtTransitionError::InvalidTransition);}
        let Some(next_epoch)=self.epoch.checked_add(1) else {
            // Never recycle an old permit after a device/session epoch wraps.
            self.phase=VtPhase::Detached;
            return Err(VtTransitionError::EpochExhausted);
        };
        self.epoch=next_epoch;
        self.phase=next;
        Ok(())
    }
    pub fn acquire(&mut self)->Result<(),VtTransitionError> {
        self.transition(VtPhase::Detached,VtPhase::Active)
    }
    pub fn begin_pause(&mut self)->Result<(),VtTransitionError> {
        self.transition(VtPhase::Active,VtPhase::Suspending)
    }
    pub fn confirm_paused(&mut self)->Result<(),VtTransitionError> {
        self.transition(VtPhase::Suspending,VtPhase::Suspended)
    }
    pub fn begin_resume(&mut self)->Result<(),VtTransitionError> {
        self.transition(VtPhase::Suspended,VtPhase::Restoring)
    }
    pub fn confirm_resources_restored(&mut self)->Result<(),VtTransitionError> {
        self.transition(VtPhase::Restoring,VtPhase::Active)
    }
    pub fn abort(&mut self) {
        // Exhaustion permanently prevents a new Active permit. The phase
        // remains detached even when abort is called repeatedly.
        self.epoch=self.epoch.saturating_add(1);
        self.phase=VtPhase::Detached;
    }
    pub fn permit(&self)->Option<VtFramePermit> {
        (self.phase==VtPhase::Active).then_some(VtFramePermit{epoch:self.epoch})
    }
    pub fn valid(&self,permit:VtFramePermit)->bool {
        self.phase==VtPhase::Active && permit.epoch==self.epoch
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_revokes_presentation_until_resources_restored() {
        let mut v=VtHandoff::default();
        assert!(v.permit().is_none());
        v.acquire().unwrap();
        let before=v.permit().unwrap();
        v.begin_pause().unwrap();
        assert!(!v.valid(before));
        assert!(v.permit().is_none());
        v.confirm_paused().unwrap();
        v.begin_resume().unwrap();
        assert!(v.permit().is_none());
        v.confirm_resources_restored().unwrap();
        assert!(!v.valid(before));
        assert!(v.permit().is_some());
    }
    #[test]
    fn exhausted_epoch_never_reissues_old_presentation_permit() {
        let mut v=VtHandoff {phase:VtPhase::Active,epoch:u64::MAX-1};
        let old=v.permit().unwrap();
        v.begin_pause().unwrap();
        assert_eq!(v.epoch(),u64::MAX);
        assert!(!v.valid(old));
        assert_eq!(v.confirm_paused(),Err(VtTransitionError::EpochExhausted));
        v.abort();
        assert_eq!(v.phase(),VtPhase::Detached);
        assert_eq!(v.acquire(),Err(VtTransitionError::EpochExhausted));
        assert!(v.permit().is_none());
    }
    #[test]
    fn exhausted_active_epoch_revokes_existing_permit_on_transition() {
        let mut v=VtHandoff {phase:VtPhase::Active,epoch:u64::MAX};
        let stale=v.permit().unwrap();
        assert_eq!(v.begin_pause(),Err(VtTransitionError::EpochExhausted));
        assert_eq!(v.phase(),VtPhase::Detached);
        assert!(!v.valid(stale));
        assert!(v.permit().is_none());
        assert_eq!(v.acquire(),Err(VtTransitionError::EpochExhausted));
    }
    #[test]
    fn out_of_order_resume_and_abort_fail_closed() {
        let mut v=VtHandoff::default();
        assert_eq!(v.begin_resume(),Err(VtTransitionError::InvalidTransition));
        v.acquire().unwrap();
        let old=v.permit().unwrap();
        assert_eq!(v.confirm_paused(),Err(VtTransitionError::InvalidTransition));
        assert!(v.valid(old));
        v.abort();
        assert!(!v.valid(old));
        assert_eq!(v.phase(),VtPhase::Detached);
    }
}
