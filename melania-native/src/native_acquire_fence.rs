//! Nonblocking Linux sync_file acquire-fence readiness for future explicit sync.
//!
//! This is a real FD readiness primitive, NOT a Wayland syncobj protocol
//! implementation, DRM presentation receipt, or permission to release wl_buffer.
//! The caller owns the fence FD and must keep it alive until GPU consumption.
use std::os::fd::{AsRawFd, BorrowedFd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireFenceState {
    Ready,
    Pending,
}

/// Query an actual Linux fence FD without blocking Wayland dispatch.
/// A sync_file signals POLLIN when all its constituent fences complete.
/// POLLERR/POLLHUP/POLLNVAL are failures, never successful GPU completion.
/// EINTR is retried only a bounded number of times, with timeout=0.
pub fn acquire_fence_state(fd: BorrowedFd<'_>) -> Result<AcquireFenceState, String> {
    let raw = fd.as_raw_fd();
    if raw < 0 { return Err("invalid acquire fence FD".into()); }
    let mut interruptions = 0u8;
    loop {
        let mut pfd = libc::pollfd { fd: raw, events: libc::POLLIN, revents: 0 };
        let result = unsafe { libc::poll(&mut pfd, 1, 0) };
        if result < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted && interruptions < 3 {
                interruptions += 1;
                continue;
            }
            return Err(format!("acquire fence poll failed: {err}"));
        }
        if result == 0 { return Ok(AcquireFenceState::Pending); }
        if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err("acquire fence FD error/hangup/invalid".into());
        }
        if pfd.revents & libc::POLLIN != 0 { return Ok(AcquireFenceState::Ready); }
        return Err("acquire fence returned unexpected poll events".into());
    }
}

/// Every acquire fence must be signaled before importing or sampling its buffer.
/// No state here asserts that a frame was submitted, displayed or released.
pub fn all_acquire_fences_ready(fds: &[BorrowedFd<'_>]) -> Result<AcquireFenceState, String> {
    // Inspect every fence: a failed fence must not be hidden indefinitely
    // behind an earlier pending fence in a multi-buffer transaction.
    let mut state = AcquireFenceState::Ready;
    for fd in fds {
        if acquire_fence_state(*fd)? == AcquireFenceState::Pending {
            state = AcquireFenceState::Pending;
        }
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn real_fd_pending_then_ready_without_waiting() {
        let (mut producer, consumer) = UnixStream::pair().unwrap();
        assert_eq!(acquire_fence_state(consumer.as_fd()).unwrap(), AcquireFenceState::Pending);
        producer.write_all(&[1]).unwrap();
        assert_eq!(acquire_fence_state(consumer.as_fd()).unwrap(), AcquireFenceState::Ready);
    }

    #[test]
    fn every_fence_must_signal() {
        let (mut a_writer, a) = UnixStream::pair().unwrap();
        let (mut b_writer, b) = UnixStream::pair().unwrap();
        a_writer.write_all(&[1]).unwrap();
        assert_eq!(all_acquire_fences_ready(&[a.as_fd(), b.as_fd()]).unwrap(), AcquireFenceState::Pending);
        b_writer.write_all(&[1]).unwrap();
        assert_eq!(all_acquire_fences_ready(&[a.as_fd(), b.as_fd()]).unwrap(), AcquireFenceState::Ready);
    }

    #[test]
    fn hangup_never_counts_as_signaled_fence() {
        let (producer, consumer) = UnixStream::pair().unwrap();
        drop(producer);
        assert!(acquire_fence_state(consumer.as_fd()).is_err());
    }

    #[test]
    fn pending_fence_does_not_hide_failed_fence() {
        let (_writer, pending) = UnixStream::pair().unwrap();
        let (writer, failed) = UnixStream::pair().unwrap();
        drop(writer);
        assert!(all_acquire_fences_ready(&[pending.as_fd(), failed.as_fd()]).is_err());
        assert!(all_acquire_fences_ready(&[failed.as_fd(), pending.as_fd()]).is_err());
    }

    #[test]
    fn kernel_eventfd_transitions_without_consuming_signal() {
        use std::os::fd::{FromRawFd, OwnedFd};
        let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        assert!(raw >= 0);
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        assert_eq!(acquire_fence_state(fd.as_fd()).unwrap(), AcquireFenceState::Pending);
        let value = 1u64.to_ne_bytes();
        assert_eq!(unsafe { libc::write(raw, value.as_ptr().cast(), value.len()) }, 8);
        for _ in 0..2 {
            assert_eq!(acquire_fence_state(fd.as_fd()).unwrap(), AcquireFenceState::Ready);
        }
        let mut received = [0u8; 8];
        assert_eq!(unsafe { libc::read(raw, received.as_mut_ptr().cast(), received.len()) }, 8);
        assert_eq!(u64::from_ne_bytes(received), 1);
        assert_eq!(acquire_fence_state(fd.as_fd()).unwrap(), AcquireFenceState::Pending);
    }

    #[test]
    fn empty_fence_set_requires_no_wait() {
        assert_eq!(all_acquire_fences_ready(&[]).unwrap(), AcquireFenceState::Ready);
    }
}
