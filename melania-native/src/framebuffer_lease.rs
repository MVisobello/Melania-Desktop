//! Stage 22: a *non-mutating* ownership ledger for FB2 GEM handle references.
//!
//! A kernel framebuffer ID is not a GEM handle. This ledger can track handles
//! acquired with DRM_IOCTL_MODE_GETFB2, and insists that every distinct handle
//! is released exactly once on the same DRM fd. It does NOT perform ioctls,
//! own a DRM fd, create FBs, or authorize a visible atomic commit.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaseError {
    InvalidFd,
    InvalidFramebuffer,
    EmptyHandles,
    DuplicateFramebuffer,
    HandleSharedAcrossFramebuffers,
    UnknownFramebuffer,
    MismatchedFd,
    ReleaseMismatch,
    OutstandingHandles,
}

#[derive(Clone, Debug, Default)]
pub struct FramebufferHandleLedger {
    leases: BTreeMap<u32, (i32, BTreeSet<u32>)>,
}

impl FramebufferHandleLedger {
    pub fn acquire(&mut self, fd: i32, framebuffer_id: u32, handles: &[u32])
        -> Result<usize, LeaseError> {
        if fd < 0 { return Err(LeaseError::InvalidFd); }
        if framebuffer_id == 0 { return Err(LeaseError::InvalidFramebuffer); }
        if self.leases.contains_key(&framebuffer_id) {
            return Err(LeaseError::DuplicateFramebuffer);
        }
        let distinct: BTreeSet<u32> = handles.iter().copied().filter(|h| *h != 0).collect();
        if distinct.is_empty() {return Err(LeaseError::EmptyHandles);}
        if self.leases.values().any(|(owner, existing)|
            *owner == fd && !existing.is_disjoint(&distinct)) {
            return Err(LeaseError::HandleSharedAcrossFramebuffers);
        }
        let count = distinct.len();
        self.leases.insert(framebuffer_id, (fd, distinct));
        Ok(count)
    }

    /// Called only after the actual DRM_IOCTL_GEM_CLOSE operations succeeded.
    /// A failed close must NOT silently erase the ledger's ownership record.
    pub fn confirm_closed(&mut self, fd: i32, framebuffer_id: u32,
        closed: &[u32]) -> Result<(), LeaseError> {
        let (owner, expected) = self.leases.get(&framebuffer_id)
            .ok_or(LeaseError::UnknownFramebuffer)?;
        if *owner != fd {return Err(LeaseError::MismatchedFd);}
        let provided: BTreeSet<u32> = closed.iter().copied().collect();
        if provided.len() != closed.len() || provided != *expected {
            return Err(LeaseError::ReleaseMismatch);
        }
        self.leases.remove(&framebuffer_id);
        Ok(())
    }

    pub fn outstanding(&self) -> usize {
        self.leases.values().map(|(_, h)| h.len()).sum()
    }

    pub fn require_empty(&self) -> Result<(), LeaseError> {
        if self.outstanding() == 0 {Ok(())}
        else {Err(LeaseError::OutstandingHandles)}
    }

    pub fn visible_kms_commit_allowed(&self) -> bool {false}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiplanar_aliases_are_closed_once() {
        let mut l=FramebufferHandleLedger::default();
        assert_eq!(l.acquire(4,12,&[7,7,8,0]).unwrap(),2);
        assert_eq!(l.confirm_closed(4,12,&[7,8]),Ok(()));
        assert_eq!(l.require_empty(),Ok(()));
        assert!(!l.visible_kms_commit_allowed());
    }
    #[test]
    fn invalid_owners_and_handles_fail_closed() {
        let mut l=FramebufferHandleLedger::default();
        assert_eq!(l.acquire(-1,1,&[1]),Err(LeaseError::InvalidFd));
        assert_eq!(l.acquire(4,0,&[1]),Err(LeaseError::InvalidFramebuffer));
        assert_eq!(l.acquire(4,1,&[0,0]),Err(LeaseError::EmptyHandles));
        l.acquire(4,1,&[3]).unwrap();
        assert_eq!(l.acquire(4,1,&[4]),Err(LeaseError::DuplicateFramebuffer));
        assert_eq!(l.acquire(4,2,&[3]),Err(LeaseError::HandleSharedAcrossFramebuffers));
        assert_eq!(l.require_empty(),Err(LeaseError::OutstandingHandles));
    }
    #[test]
    fn wrong_fd_and_partial_release_preserve_outstanding_state() {
        let mut l=FramebufferHandleLedger::default();
        l.acquire(4,3,&[8,9]).unwrap();
        assert_eq!(l.confirm_closed(5,3,&[8,9]),Err(LeaseError::MismatchedFd));
        assert_eq!(l.confirm_closed(4,3,&[8]),Err(LeaseError::ReleaseMismatch));
        assert_eq!(l.confirm_closed(4,3,&[8,8,9]),Err(LeaseError::ReleaseMismatch));
        assert_eq!(l.outstanding(),2);
        assert_eq!(l.confirm_closed(4,3,&[9,8]),Ok(()));
        assert_eq!(l.outstanding(),0);
    }
    #[test]
    fn same_handle_number_on_different_fds_is_not_same_gem_reference() {
        let mut l=FramebufferHandleLedger::default();
        l.acquire(4,11,&[99]).unwrap();
        l.acquire(5,12,&[99]).unwrap();
        assert_eq!(l.outstanding(),2);
        l.confirm_closed(4,11,&[99]).unwrap();
        l.confirm_closed(5,12,&[99]).unwrap();
        assert_eq!(l.require_empty(),Ok(()));
    }
}
