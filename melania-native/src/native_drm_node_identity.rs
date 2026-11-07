//! Bind a libseat-owned DRM FD to the actual udev-discovered kernel node.
//!
//! The selected /dev/dri pathname is not sufficient: it may be replaced
//! during discovery, VT loss or GBM/EGL initialization. Verify that the
//! libseat-issued FD and the selected path refer to the same character-device
//! inode on the same filesystem with the same Linux device number.
//! No direct DRM open or modeset is performed by this checker.
//! A successful result is NOT proof of DRM master, native login or rollback.

use std::{
    fs::{self, File, Metadata},
    os::{
        fd::OwnedFd,
        unix::fs::{FileTypeExt, MetadataExt},
    },
    path::Path,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmNodeIdentity {
    rdev: u64,
    filesystem_device: u64,
    inode: u64,
}

impl DrmNodeIdentity {
    /// The kernel's character-device number; not evidence of DRM master.
    pub fn rdev(self) -> u64 { self.rdev }
}

fn char_identity(metadata: &Metadata, source: &str) -> Result<DrmNodeIdentity,String> {
    if !metadata.file_type().is_char_device() {
        return Err(format!("{source} is not a character device"));
    }
    Ok(DrmNodeIdentity {
        rdev: metadata.rdev(),
        filesystem_device: metadata.dev(),
        inode: metadata.ino(),
    })
}

/// Inspect a clone of the real libseat-issued descriptor, not a newly
/// opened /dev/dri path. On any Err the caller MUST close the original
/// seat-owned FD and refuse to present a frame.
/// The previous identity is the baseline before a VT pause/reacquisition.
pub fn verify_libseat_drm_node(
    path: &Path,
    seat_fd: &OwnedFd,
    previous: Option<DrmNodeIdentity>,
) -> Result<DrmNodeIdentity,String> {
    let cloned=seat_fd.try_clone()
        .map_err(|e|format!("Cannot clone libseat DRM FD for fstat: {e}"))?;
    let fd_metadata=File::from(cloned).metadata()
        .map_err(|e|format!("Cannot fstat the libseat DRM FD: {e}"))?;
    let path_metadata=fs::metadata(path)
        .map_err(|e|format!("Cannot stat discovered DRM node {}: {e}",path.display()))?;
    let actual=char_identity(&fd_metadata,"Libseat DRM descriptor")?;
    let discovered=char_identity(&path_metadata,"Discovered DRM device")?;
    if actual!=discovered {
        return Err("Libseat DRM descriptor differs from udev-discovered device node".into());
    }
    if previous.is_some_and(|baseline|baseline!=actual) {
        return Err("DRM character device was replaced across seat recovery".into());
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn null_fd() -> OwnedFd {
        File::open("/dev/null").expect("Linux /dev/null").into()
    }

    #[test]
    fn real_char_device_fd_matches_the_kernel_node() {
        let fd=null_fd();
        let identity=verify_libseat_drm_node(Path::new("/dev/null"),&fd,None).unwrap();
        assert_ne!(identity.rdev(),0);
        assert_eq!(
            verify_libseat_drm_node(Path::new("/dev/null"),&fd,Some(identity)),
            Ok(identity)
        );
    }

    #[test]
    fn another_live_char_device_does_not_match_fd() {
        let fd=null_fd();
        assert!(verify_libseat_drm_node(Path::new("/dev/zero"),&fd,None).is_err());
    }

    #[test]
    fn old_inode_is_rejected_even_if_dev_t_stays_the_same() {
        let fd=null_fd();
        let first=verify_libseat_drm_node(Path::new("/dev/null"),&fd,None).unwrap();
        let replaced=DrmNodeIdentity { inode:first.inode.wrapping_add(1), ..first };
        assert!(verify_libseat_drm_node(Path::new("/dev/null"),&fd,Some(replaced)).is_err());
    }

    #[test]
    fn a_regular_file_cannot_be_used_as_a_drm_card() {
        let fd=null_fd();
        let executable=std::env::current_exe().expect("test executable");
        assert!(verify_libseat_drm_node(&executable,&fd,None).is_err());
    }
}
