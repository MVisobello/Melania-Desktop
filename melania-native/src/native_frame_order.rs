//! Real KMS stream frame-order validation shared by CPU and GBM presenters.
//!
//! A persistent compositor may need to repaint exactly the same retained
//! scene generation (for example after an output expose or a forced full
//! redraw). Rejecting that legitimate repaint terminates the actual DRM
//! stream even though the sender carries a NEW scheduler/seat sequence.
//!
//! A new sequence is essential. Neither duplicate/reordered messages nor
//! a new device epoch can be accepted by an existing DRM/card owner.
//! Re-acquisition must rebuild the libseat/GBM/KMS resources first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameIdentity {
    pub epoch: u64,
    pub sequence: u64,
    pub generation: u64,
}

impl FrameIdentity {
    pub fn new(epoch:u64,sequence:u64,generation:u64)->Self {
        Self {epoch,sequence,generation}
    }

    /// True only if this live hardware owner may attempt the new frame.
    /// Does NOT authorize a Wayland ACK; the real KMS flip, event and
    /// connector -> CRTC -> framebuffer verification must still succeed.
    pub fn accepted_after(self, previous:Option<Self>, persistent:bool)->bool {
        if self.sequence==0 {return false;}
        match previous {
            None=>true,
            Some(old)=>self.epoch==old.epoch &&
                self.sequence>old.sequence &&
                (self.generation>old.generation ||
                 (persistent && self.generation==old.generation)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrameIdentity as Id;
    #[test]
    fn persistent_repeat_scene_requires_fresh_sequence_and_same_device_epoch() {
        let first=Id::new(7,100,42);
        assert!(first.accepted_after(None,true));
        assert!(Id::new(7,101,42).accepted_after(Some(first),true));
        assert!(!Id::new(7,101,42).accepted_after(Some(first),false));
        assert!(!Id::new(7,100,42).accepted_after(Some(first),true));
        assert!(!Id::new(7,99,43).accepted_after(Some(first),true));
        assert!(!Id::new(7,101,41).accepted_after(Some(first),true));
        assert!(!Id::new(8,101,43).accepted_after(Some(first),true));
        assert!(!Id::new(6,101,43).accepted_after(Some(first),true));
        assert!(!Id::new(7,0,43).accepted_after(Some(first),true));
    }
    #[test]
    fn both_presenters_preserve_strict_finite_virtual_gpu_pixel_proofs() {
        let initial=Id::new(1,1,1);
        assert!(Id::new(1,2,2).accepted_after(Some(initial),false));
        assert!(Id::new(1,2,3).accepted_after(Some(initial),true));
        assert!(!Id::new(1,2,1).accepted_after(Some(initial),false));
        assert!(!Id::new(1,2,0).accepted_after(Some(initial),true));
        assert!(!Id::new(1,0,1).accepted_after(None,false));
    }
    #[test]
    fn thousands_of_persistent_repaints_do_not_require_new_scene_mutations() {
        let mut old=Id::new(11,1,100);
        for sequence in 2..=4096 {
            let next=Id::new(11,sequence,100);
            assert!(next.accepted_after(Some(old),true));
            old=next;
        }
        assert!(!old.accepted_after(Some(old),true));
        assert!(Id::new(11,4097,101).accepted_after(Some(old),true));
    }
}
