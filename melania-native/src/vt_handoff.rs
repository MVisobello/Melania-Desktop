//! VT lifecycle policy; actual ioctls/session FD ownership are NOT wired.
//! Every pause, resume or abort revokes old presentation permits.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum VtPhase {Detached,Active,Suspending,Suspended,Restoring}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct VtFramePermit {epoch:u64}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum VtTransitionError {InvalidTransition}

#[derive(Debug)]
pub struct VtHandoff {phase:VtPhase,epoch:u64}
impl Default for VtHandoff {
    fn default()->Self {Self {phase:VtPhase::Detached,epoch:0}}
}
impl VtHandoff {
    pub fn phase(&self)->VtPhase {self.phase}
    pub fn epoch(&self)->u64 {self.epoch}
    fn transition(&mut self,expected:VtPhase,next:VtPhase)->Result<(),VtTransitionError> {
        if self.phase!=expected {return Err(VtTransitionError::InvalidTransition);}
        self.epoch=self.epoch.wrapping_add(1);
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
        self.epoch=self.epoch.wrapping_add(1);
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
