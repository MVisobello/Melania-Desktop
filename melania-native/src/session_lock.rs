//! Session lock sequencing: a state machine, not a secure lockscreen.
//! The UI must cover ALL outputs and input isolation must be confirmed
//! before a session may report Locked. No password handling occurs here.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LockPhase {Unlocked,LockRequested,Locked,UnlockRequested}
#[derive(Debug,Default)]
pub struct SessionLock {
    phase:LockPhase,revision:u64,
}
impl Default for LockPhase {fn default()->Self {Self::Unlocked}}
impl SessionLock {
    pub fn phase(&self)->LockPhase {self.phase}
    pub fn revision(&self)->u64 {self.revision}
    fn set(&mut self,new:LockPhase) {
        self.phase=new;
        self.revision=self.revision.wrapping_add(1);
    }
    pub fn request_lock(&mut self)->bool {
        if self.phase!=LockPhase::Unlocked {return false;}
        self.set(LockPhase::LockRequested);true
    }
    pub fn confirm_outputs_covered(&mut self,all_outputs:bool,input_isolated:bool)->bool{
        if self.phase!=LockPhase::LockRequested || !all_outputs || !input_isolated {
            return false;
        }
        self.set(LockPhase::Locked);true
    }
    /// Authentication is external to this reducer; false never unlocks.
    pub fn request_unlock(&mut self,authenticated:bool)->bool {
        if self.phase!=LockPhase::Locked || !authenticated {return false;}
        self.set(LockPhase::UnlockRequested);true
    }
    pub fn confirm_unlocked(&mut self)->bool {
        if self.phase!=LockPhase::UnlockRequested {return false;}
        self.set(LockPhase::Unlocked);true
    }
    /// A VT pause or output hotplug must cancel unlock and require
    /// verification of screen coverage before processing further input.
    pub fn revoke_on_output_change(&mut self){
        if self.phase!=LockPhase::LockRequested {
            self.set(LockPhase::LockRequested);
        }
    }
    pub fn input_blocked(&self)->bool {self.phase!=LockPhase::Unlocked}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_requires_full_output_coverage_and_isolated_input() {
        let mut s=SessionLock::default();
        assert!(s.request_lock());
        assert!(s.input_blocked());
        assert!(!s.confirm_outputs_covered(true,false));
        assert!(!s.confirm_outputs_covered(false,true));
        assert_eq!(s.phase(),LockPhase::LockRequested);
        assert!(s.confirm_outputs_covered(true,true));
        assert!(!s.request_unlock(false));
        assert!(s.request_unlock(true));
        assert!(s.input_blocked());
        assert!(s.confirm_unlocked());
        assert!(!s.input_blocked());
    }
    #[test]
    fn hotplug_cancels_unlock_and_requires_fresh_coverage() {
        let mut s=SessionLock::default();
        s.request_lock();
        s.confirm_outputs_covered(true,true);
        s.request_unlock(true);
        s.revoke_on_output_change();
        assert_eq!(s.phase(),LockPhase::LockRequested);
        assert!(!s.confirm_unlocked());
        assert!(s.input_blocked());
    }
}
