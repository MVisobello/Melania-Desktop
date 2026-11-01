//! Stage 14: associate a native DRM preflight with one libseat/udev epoch.
//!
//! Capturing a DRM descriptor is not sufficient: the seat can pause, or the
//! udev device can disappear, while an EGL operation is underway. This CPU
//! check must be re-evaluated after processing actual libseat/udev events.
//! No kernel object ownership, VT restore or KMS scanout is implemented.
use std::path::{Path, PathBuf};
use crate::NativeLifecycle;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceEpoch {
    id: u64,
    path: PathBuf,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceEpochError {
    SeatInactive,
    DeviceMissingOrReplaced,
    SessionOrGpuChanged,
}

impl DeviceEpoch {
    pub fn capture(
        state: &NativeLifecycle,
        id: u64,
        path: &Path,
    ) -> Result<Self, DeviceEpochError> {
        if !state.seat_active() {
            return Err(DeviceEpochError::SeatInactive);
        }
        if state.devices().get(&id).is_none_or(|current|current != path) {
            return Err(DeviceEpochError::DeviceMissingOrReplaced);
        }
        Ok(Self {
            id, path: path.to_path_buf(),
            generation: state.frame_gate().generation(),
        })
    }

    pub fn verify(&self, state: &NativeLifecycle) -> Result<(), DeviceEpochError> {
        if !state.seat_active() {
            return Err(DeviceEpochError::SeatInactive);
        }
        if state.frame_gate().generation() != self.generation {
            return Err(DeviceEpochError::SessionOrGpuChanged);
        }
        if state.devices().get(&self.id) != Some(&self.path) {
            return Err(DeviceEpochError::DeviceMissingOrReplaced);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceChange;
    use smithay::backend::session::Event;

    fn active() -> NativeLifecycle {
        let mut state=NativeLifecycle::default();
        state.on_session(Event::ActivateSession);
        state.on_device(DeviceChange::Added{
            id:11,path:"/dev/dri/card0".into()
        });
        state
    }

    #[test]
    fn stable_seat_and_gpu_epoch_accept_a_read_only_checkpoint() {
        let state=active();
        let epoch=DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")).unwrap();
        assert_eq!(epoch.verify(&state),Ok(()));
        assert!(!state.can_present_kms());
    }

    #[test]
    fn pause_resume_never_revalidates_a_previous_device_epoch() {
        let mut state=active();
        let epoch=DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")).unwrap();
        state.on_session(Event::PauseSession);
        assert_eq!(epoch.verify(&state),Err(DeviceEpochError::SeatInactive));
        state.on_session(Event::ActivateSession);
        assert_eq!(epoch.verify(&state),Err(DeviceEpochError::SessionOrGpuChanged));
    }

    #[test]
    fn gpu_hotplug_change_and_remove_invalidate_the_epoch() {
        let mut state=active();
        let epoch=DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")).unwrap();
        state.on_device(DeviceChange::Changed{id:11});
        assert_eq!(epoch.verify(&state),Err(DeviceEpochError::SessionOrGpuChanged));
        let new_epoch=DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")).unwrap();
        state.on_device(DeviceChange::Removed{id:11});
        assert!(new_epoch.verify(&state).is_err());
        assert_eq!(
            DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")),
            Err(DeviceEpochError::DeviceMissingOrReplaced)
        );
    }

    #[test]
    fn reused_id_with_another_device_path_is_never_accepted() {
        let mut state=active();
        let old=DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")).unwrap();
        state.on_device(DeviceChange::Added{id:11,path:"/dev/dri/card1".into()});
        assert!(old.verify(&state).is_err());
        assert_eq!(
            DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")),
            Err(DeviceEpochError::DeviceMissingOrReplaced)
        );
    }

    #[test]
    fn cannot_begin_checkpoints_with_an_inactive_seat() {
        let mut state=active();
        state.on_session(Event::PauseSession);
        assert_eq!(
            DeviceEpoch::capture(&state,11,Path::new("/dev/dri/card0")),
            Err(DeviceEpochError::SeatInactive)
        );
    }
}
