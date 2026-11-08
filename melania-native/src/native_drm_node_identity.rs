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
    // Inspect the path itself, not its target. A substituted cardN symlink
    // could otherwise point at the same character device as the seat FD and
    // silently pass an identity check that was intended for a udev-owned node.
    let first_path_metadata=fs::symlink_metadata(path)
        .map_err(|e|format!("Cannot lstat discovered DRM node {}: {e}",path.display()))?;
    let actual=char_identity(&fd_metadata,"Libseat DRM descriptor")?;
    let discovered=char_identity(&first_path_metadata,"Discovered DRM device")?;
    if actual!=discovered {
        return Err("Libseat DRM descriptor differs from udev-discovered device node".into());
    }
    if previous.is_some_and(|baseline|baseline!=actual) {
        return Err("DRM character device was replaced across seat recovery".into());
    }
    // The first pathname read and fstat are separate syscalls. Revalidate
    // the original descriptor AND the discovered pathname before accepting
    // the seat resource. This catches replacement during the identity probe;
    // it is not a substitute for GPU hotplug/VT epoch revocation.
    let final_fd=File::from(seat_fd.try_clone()
        .map_err(|e|format!("Cannot clone libseat DRM FD for final fstat: {e}"))?);
    let final_fd_identity=char_identity(&final_fd.metadata()
        .map_err(|e|format!("Cannot re-fstat libseat DRM FD: {e}"))?,"Final libseat DRM descriptor")?;
    let final_path_identity=char_identity(&fs::symlink_metadata(path)
        .map_err(|e|format!("Cannot re-lstat discovered DRM node {}: {e}",path.display()))?,
        "Final discovered DRM device")?;
    if final_fd_identity!=actual || final_path_identity!=actual {
        return Err("DRM descriptor or device node changed during libseat identity verification".into());
    }
    Ok(actual)
}

/// Validate the *DRM primary card* rather than an arbitrary character node.
/// Linux DRM reserves character-device major 226; cardN primary nodes occupy
/// minor 0..63. Render nodes (minor >=128) and control nodes are not valid
/// KMS scanout descriptors. Never allow a substituted /dev/null, tty or
/// renderD* FD to pass just because both path and descriptor agree.
pub fn verify_libseat_drm_primary_card(
    path: &Path,
    seat_fd: &OwnedFd,
    previous: Option<DrmNodeIdentity>,
) -> Result<DrmNodeIdentity,String> {
    let node=verify_libseat_drm_node(path,seat_fd,previous)?;
    // Linux dev_t encoding, see include/linux/kdev_t.h and sysmacros.h.
    let major=((node.rdev >> 8) & 0xfff) | ((node.rdev >> 32) & !0xfff);
    let minor=(node.rdev & 0xff) | ((node.rdev >> 12) & !0xff);
    if major!=226 || minor>=64 {
        return Err(format!("Libseat descriptor is not a DRM primary KMS card (major={major}, minor={minor})"));
    }
    let filename=path.file_name().and_then(|name|name.to_str())
        .ok_or("Selected DRM primary card path has no UTF-8 basename")?;
    let Some(number)=filename.strip_prefix("card") else {
        return Err("Selected DRM device is not a cardN primary node".into());
    };
    if number.is_empty() || !number.bytes().all(|byte|byte.is_ascii_digit()) ||
       number.parse::<u64>().ok()!=Some(minor) {
        return Err("Selected DRM cardN name does not match its kernel minor number".into());
    }
    Ok(node)
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
    fn a_non_drm_character_device_is_rejected_even_if_fd_matches_path() {
        let fd=null_fd();
        assert!(verify_libseat_drm_primary_card(Path::new("/dev/null"),&fd,None).is_err());
    }

    #[test]
    fn an_unrelated_character_device_cannot_masquerade_as_a_kms_card() {
        let fd=File::open("/dev/zero").expect("Linux /dev/zero");
        assert!(verify_libseat_drm_primary_card(Path::new("/dev/zero"),&fd.into(),None).is_err());
    }

    #[test]
    fn symlink_to_same_live_char_device_is_not_a_trusted_drm_node() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64=AtomicU64::new(0);
        let path=std::env::temp_dir().join(format!(
            "melania-drm-node-link-{}-{}",std::process::id(),
            NEXT.fetch_add(1,Ordering::Relaxed)));
        symlink("/dev/null",&path).expect("create temporary symlink");
        let fd=null_fd();
        let outcome=verify_libseat_drm_node(&path,&fd,None);
        std::fs::remove_file(&path).expect("remove temporary symlink");
        assert!(outcome.is_err(),"symlink to the exact seat FD must fail closed");
    }

    #[test]
    fn repeated_identity_probe_does_not_reject_unchanged_real_device() {
        let fd=null_fd();
        let first=verify_libseat_drm_node(Path::new("/dev/null"),&fd,None).unwrap();
        for _ in 0..16 {
            assert_eq!(verify_libseat_drm_node(Path::new("/dev/null"),&fd,Some(first)),Ok(first));
        }
    }

    #[test]
    fn a_regular_file_cannot_be_used_as_a_drm_card() {
        let fd=null_fd();
        let executable=std::env::current_exe().expect("test executable");
        assert!(verify_libseat_drm_node(&executable,&fd,None).is_err());
    }
}
