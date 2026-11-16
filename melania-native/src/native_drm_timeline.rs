//! Kernel DRM syncobj timeline operations for the explicit-sync backend.
//!
//! All handles belong to one live DRM file description. This is not the
//! Wayland explicit-sync global, a client-buffer release, or KMS evidence.
//! The owner must keep its libseat-issued DRM descriptor valid; destroy
//! timelines before releasing the seat. The probe does not change scanout.
use drm::{Device,DriverCapability,control::{Device as ControlDevice,syncobj}};
use smithay::backend::drm::DrmDeviceFd;
use std::{io,os::fd::{AsFd,BorrowedFd,FromRawFd,OwnedFd}};
use crate::native_acquire_fence::{acquire_fence_state,AcquireFenceState};

pub struct KernelTimeline<'a> {
    card:&'a DrmDeviceFd,
    handle:Option<syncobj::Handle>,
}
impl<'a> KernelTimeline<'a> {
    pub fn create(card:&'a DrmDeviceFd)->io::Result<Self> {
        Ok(Self {card,handle:Some(card.create_syncobj(false)?)})
    }
    pub fn import(card:&'a DrmDeviceFd,fd:BorrowedFd<'_>)->io::Result<Self> {
        Ok(Self {card,handle:Some(card.fd_to_syncobj(fd,false)?)})
    }
    fn handle(&self)->syncobj::Handle {
        self.handle.expect("live DRM timeline handle")
    }
    pub fn export_timeline(&self)->io::Result<OwnedFd> {
        self.card.syncobj_to_fd(self.handle(),false)
    }
    pub fn signalled_point(&self)->io::Result<u64> {
        let mut points=[0];
        self.card.syncobj_timeline_query(&[self.handle()],&mut points,false)?;
        Ok(points[0])
    }
    pub fn ready(&self,point:u64)->io::Result<bool> {
        Ok(self.signalled_point()? >= point)
    }
    /// Signal only a point whose real producer/consumer completion the
    /// caller has independently established. Never infer completion here.
    pub fn signal(&self,point:u64)->io::Result<()> {
        if point==0 || point<=self.signalled_point()? {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"non-increasing DRM timeline point"));
        }
        self.card.syncobj_timeline_signal(&[self.handle()],&[point])
    }
    /// Kernel eventfd registration for wl_surface transaction blockers.
    /// No blocking wait on the Wayland dispatch thread.
    pub fn eventfd(&self,point:u64)->io::Result<OwnedFd> {
        let raw=unsafe {libc::eventfd(0,libc::EFD_CLOEXEC|libc::EFD_NONBLOCK)};
        if raw<0 {return Err(io::Error::last_os_error());}
        let fd=unsafe {OwnedFd::from_raw_fd(raw)};
        self.card.syncobj_eventfd(self.handle(),point,fd.as_fd(),false)?;
        Ok(fd)
    }
    /// Transfer an available timeline point to a temporary binary syncobj.
    /// Export a real sync_file and destroy the temporary handle even on
    /// export failure. The returned FD independently owns the fence.
    pub fn export_sync_file(&self,point:u64)->io::Result<OwnedFd> {
        let binary=Self::create(self.card)?;
        self.card.syncobj_timeline_transfer(self.handle(),binary.handle(),point,0)?;
        let fd=self.card.syncobj_to_fd(binary.handle(),true)?;
        binary.close()?;
        Ok(fd)
    }
    pub fn close(mut self)->io::Result<()> {
        if let Some(handle)=self.handle.take() {self.card.destroy_syncobj(handle)?;}
        Ok(())
    }
}
impl Drop for KernelTimeline<'_> {
    fn drop(&mut self) {
        if let Some(handle)=self.handle.take() {
            if let Err(error)=self.card.destroy_syncobj(handle) {
                eprintln!("DRM syncobj cleanup failed: {error}");
            }
        }
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum TimelineProbe { Unsupported, TimelineOnly, TimelineAndEventfd }

/// Test real DRM handles, imported timeline FD and two independently
/// signalled points. This deliberately signals ONLY test-owned objects,
/// never a live Wayland client's release point.
pub fn probe_kernel_timelines(card:&DrmDeviceFd)->Result<TimelineProbe,Box<dyn std::error::Error>> {
    if card.get_driver_capability(DriverCapability::SyncObj)?==0 ||
       card.get_driver_capability(DriverCapability::TimelineSyncObj)?==0 {
        return Ok(TimelineProbe::Unsupported);
    }
    let producer=KernelTimeline::create(card)?;
    let peer_fd=producer.export_timeline()?;
    let consumer=KernelTimeline::import(card,peer_fd.as_fd())?;
    let release=KernelTimeline::create(card)?;
    if consumer.ready(2)? || release.ready(7)? {
        return Err("unsubmitted DRM timeline unexpectedly signalled".into());
    }
    // Some virtual drivers support timeline objects but not the newer
    // eventfd ioctl. That capability is explicitly NOT a successful blocker.
    let event=match consumer.eventfd(2) {
        Ok(fd)=>Some(fd),
        Err(err) if matches!(err.raw_os_error(),Some(libc::ENOTTY)|Some(libc::EOPNOTSUPP)|Some(libc::ENOSYS))=>None,
        Err(err)=>return Err(err.into()),
    };
    if let Some(fd)=event.as_ref() {
        if acquire_fence_state(fd.as_fd())?!=AcquireFenceState::Pending {
            return Err("DRM acquire eventfd was ready before timeline signal".into());
        }
    }
    producer.signal(1)?;
    if !consumer.ready(1)? || consumer.ready(2)? || release.ready(7)? {
        return Err("imported DRM timeline did not preserve point ordering".into());
    }
    producer.signal(2)?;
    if !consumer.ready(2)? || release.ready(7)? {
        return Err("DRM acquire completion incorrectly signalled release timeline".into());
    }
    if let Some(fd)=event.as_ref() {
        // Kernel workqueue signalling can be asynchronous. Wait in this
        // isolated capability probe only, never in a Wayland commit hook.
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(1);
        while acquire_fence_state(fd.as_fd())?==AcquireFenceState::Pending {
            if std::time::Instant::now()>=deadline {
                return Err("DRM syncobj eventfd did not signal".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    let sync_file=consumer.export_sync_file(2)?;
    if acquire_fence_state(sync_file.as_fd())?!=AcquireFenceState::Ready {
        return Err("exported DRM acquire sync_file not signalled".into());
    }
    release.signal(7)?;
    if !release.ready(7)? || consumer.ready(3)? || release.signal(7).is_ok() {
        return Err("DRM release timeline monotonicity/isolation failed".into());
    }
    let support=if event.is_some() {TimelineProbe::TimelineAndEventfd}
        else {TimelineProbe::TimelineOnly};
    // The FD import is a distinct kernel syncobj handle. Explicitly retire
    // the full probe before the calling libseat owner closes its device.
    drop(sync_file);
    drop(event);
    drop(peer_fd);
    consumer.close()?;
    producer.close()?;
    release.close()?;
    Ok(support)
}
