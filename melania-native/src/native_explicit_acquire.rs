//! Nonblocking Smithay transaction gate for explicit DRM acquire points.
//! An eventfd wake-up alone is not proof of GPU completion: Smithay may
//! wake its timeline eventfds when a device is invalidated. Always check
//! the original seat token and the actual timeline point before releasing.
use std::{os::fd::{AsFd,BorrowedFd,OwnedFd},sync::{Arc,atomic::{AtomicU8,Ordering}}};
use smithay::wayland::{
    compositor::{Blocker,BlockerState,add_blocker},
    drm_syncobj::DrmSyncPoint,
};
use smithay::backend::renderer::sync::Fence;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use crate::{
    native_acquire_fence::{acquire_fence_state,AcquireFenceState},
    native_presentation_guard::{NativePresentationGuard,PresentationToken},
};

const PENDING:u8=0;
const RELEASED:u8=1;
const CANCELLED:u8=2;

#[derive(Clone,Debug)]
pub struct ExplicitAcquireBlocker(Arc<AtomicU8>);
impl Blocker for ExplicitAcquireBlocker {
    fn state(&self)->BlockerState {
        match self.0.load(Ordering::Acquire) {
            RELEASED=>BlockerState::Released,
            CANCELLED=>BlockerState::Cancelled,
            _=>BlockerState::Pending,
        }
    }
}
impl ExplicitAcquireBlocker {
    fn resolve(&self,state:u8) {
        // Never revive a cancelled transaction on a late kernel event.
        let _=self.0.compare_exchange(PENDING,state,Ordering::AcqRel,Ordering::Acquire);
    }
}

pub struct AcquireCommitGate {
    token:PresentationToken,
    blocker:ExplicitAcquireBlocker,
}
impl AcquireCommitGate {
    pub fn new(token:PresentationToken)->Self {
        Self {token,blocker:ExplicitAcquireBlocker(Arc::new(AtomicU8::new(PENDING)))}
    }
    pub fn blocker(&self)->ExplicitAcquireBlocker {self.blocker.clone()}
    pub fn cancel(&self) {self.blocker.resolve(CANCELLED);}
    /// Check an actual event FD and re-query its timeline after the wake.
    /// A revoked seat, failed poll, or false wake cancels rather than
    /// applying the transaction. The caller must notify blocker_cleared
    /// once a terminal state has been returned.
    pub fn observe(
        &self,guard:&NativePresentationGuard,seat_active:bool,
        fd:BorrowedFd<'_>,verify_point:impl FnOnce()->Result<bool,String>,
    )->Result<BlockerState,String> {
        if !guard.accepts(self.token,seat_active) {
            self.cancel();
            return Ok(self.blocker.state());
        }
        if self.blocker.state()!=BlockerState::Pending {
            return Ok(self.blocker.state());
        }
        let ready=match acquire_fence_state(fd) {
            Ok(state)=>state,
            Err(error)=>{self.cancel();return Err(error);}
        };
        if ready==AcquireFenceState::Pending {
            return Ok(BlockerState::Pending);
        }
        match verify_point() {
            Ok(true)=>self.blocker.resolve(RELEASED),
            Ok(false)=>{
                self.cancel();
                return Err("acquire eventfd woke without a verified DRM timeline point".into());
            }
            Err(error)=>{self.cancel();return Err(error);}
        }
        Ok(self.blocker.state())
    }
}
impl Drop for AcquireCommitGate {
    fn drop(&mut self) {self.cancel();}
}

/// Adapter for Smithay's real wp_linux_drm_syncobj_surface_v1 cached point.
/// It can be registered in a pre-commit hook and driven by calloop without
/// blocking dispatch. The caller must keep it until terminal resolution,
/// wake the client's transaction queue, and independently own release
/// points until GPU consumption. This adapter does not advertise a global.
pub struct WaylandAcquireGate {
    point:DrmSyncPoint,
    event:Arc<OwnedFd>,
    gate:AcquireCommitGate,
}
impl WaylandAcquireGate {
    pub fn install(surface:&WlSurface,point:DrmSyncPoint,token:PresentationToken)
        ->Result<Self,String> {
        let gate=AcquireCommitGate::new(token);
        let event=point.eventfd().map_err(|error|format!("DRM acquire eventfd: {error}"))?;
        add_blocker(surface,gate.blocker());
        Ok(Self {point,event,gate})
    }
    pub fn event_fd(&self)->&OwnedFd {self.event.as_ref()}
    pub fn observe(&self,guard:&NativePresentationGuard,seat_active:bool)
        ->Result<BlockerState,String> {
        self.gate.observe(guard,seat_active,self.event.as_fd(),
            ||Ok(Fence::is_signaled(&self.point)))
    }
    pub fn cancel(&self) {self.gate.cancel();}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    fn armed()->(NativePresentationGuard,PresentationToken) {
        let mut guard=NativePresentationGuard::default();
        guard.arm_initial(true).unwrap();
        let token=guard.ticket(true).unwrap();
        (guard,token)
    }
    #[test]
    fn actual_fd_wait_then_verified_point_releases_transaction() {
        let (guard,token)=armed();
        let gate=AcquireCommitGate::new(token);
        let (mut producer,fd)=UnixStream::pair().unwrap();
        let blocker=gate.blocker();
        assert_eq!(gate.observe(&guard,true,fd.as_fd(),||panic!("pending FD queried")).unwrap(),BlockerState::Pending);
        producer.write_all(&[1]).unwrap();
        assert_eq!(gate.observe(&guard,true,fd.as_fd(),||Ok(true)).unwrap(),BlockerState::Released);
        assert_eq!(blocker.state(),BlockerState::Released);
    }
    #[test]
    fn false_wake_cancels_even_when_fd_is_ready() {
        let (guard,token)=armed();
        let gate=AcquireCommitGate::new(token);
        let (mut producer,fd)=UnixStream::pair().unwrap();
        producer.write_all(&[1]).unwrap();
        assert!(gate.observe(&guard,true,fd.as_fd(),||Ok(false)).is_err());
        assert_eq!(gate.observe(&guard,true,fd.as_fd(),||Ok(true)).unwrap(),BlockerState::Cancelled);
    }
    #[test]
    fn revoked_seat_cannot_release_late_acquire() {
        let (mut guard,token)=armed();
        let gate=AcquireCommitGate::new(token);
        let (mut producer,fd)=UnixStream::pair().unwrap();
        guard.revoke();
        producer.write_all(&[1]).unwrap();
        assert_eq!(gate.observe(&guard,true,fd.as_fd(),||panic!("revoked seat queried")).unwrap(),BlockerState::Cancelled);
    }
    #[test]
    fn hangup_and_abandoned_gate_cancel_pending_transaction() {
        let (guard,token)=armed();
        let gate=AcquireCommitGate::new(token);
        let (producer,fd)=UnixStream::pair().unwrap();
        drop(producer);
        assert!(gate.observe(&guard,true,fd.as_fd(),||Ok(true)).is_err());
        assert_eq!(gate.blocker().state(),BlockerState::Cancelled);
        let abandoned=AcquireCommitGate::new(token);
        let blocker=abandoned.blocker();
        drop(abandoned);
        assert_eq!(blocker.state(),BlockerState::Cancelled);
    }
}
