//! Stage 1 native seat/udev lifecycle for Melania Desktop.
//!
//! This is actual Smithay libseat/udev integration, deliberately separate from
//! the existing winit compositor. It does not acquire DRM framebuffers, open
//! libinput devices, switch VTs, or advertise a functioning native session.
pub mod outputs;
pub mod renderer;
pub mod planes;
pub mod frame_gate;
use std::{collections::BTreeMap, path::PathBuf};
use smithay::backend::{
    session::Event as SessionEvent,
    udev::UdevEvent,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceChange {
    Added { id: u64, path: PathBuf },
    Changed { id: u64 },
    Removed { id: u64 },
}

/// The state machine persists across seat pause/resume and GPU hotplug.
/// No DRM rendering is ever permitted by this stage1 type.
#[derive(Debug, Default)]
pub struct NativeLifecycle {
    seat_active: bool,
    devices: BTreeMap<u64, PathBuf>,
    events: u64,
    frame_gate: frame_gate::NativeFrameGate,
}

impl NativeLifecycle {
    pub fn seat_active(&self) -> bool { self.seat_active }
    pub fn devices(&self) -> &BTreeMap<u64, PathBuf> { &self.devices }
    pub fn event_count(&self) -> u64 { self.events }
    pub fn frame_gate(&self)->&frame_gate::NativeFrameGate {&self.frame_gate}

    pub fn on_session(&mut self, event: SessionEvent) {
        self.events = self.events.saturating_add(1);
        match event {
            SessionEvent::PauseSession => {
                // When the native renderer is implemented, this transition
                // must suspend libinput and disable all KMS output managers.
                self.seat_active = false;
                self.frame_gate.set_seat_active(false);
            }
            SessionEvent::ActivateSession => {
                // Resume must restore all devices before any rendering.
                self.seat_active = true;
                self.frame_gate.set_seat_active(true);
            }
        }
    }

    pub fn on_device(&mut self, event: DeviceChange) {
        self.events = self.events.saturating_add(1);
        // Any GPU hotplug/change invalidates the pending render candidate.
        self.frame_gate.invalidate();
        match event {
            DeviceChange::Added { id, path } => {self.devices.insert(id, path);}
            DeviceChange::Changed { .. } => {
                // A change requires a fresh DRM connector reprobe in stage2.
                // We deliberately do not attempt KMS calls here.
            }
            DeviceChange::Removed { id } => {self.devices.remove(&id);}
        }
    }

    pub fn on_udev(&mut self, event: UdevEvent) {
        let change=match event {
            UdevEvent::Added { device_id, path } => DeviceChange::Added {
                id: device_id as u64, path,
            },
            UdevEvent::Changed { device_id } => DeviceChange::Changed {
                id: device_id as u64,
            },
            UdevEvent::Removed { device_id } => DeviceChange::Removed {
                id: device_id as u64,
            },
        };
        self.on_device(change);
    }

    pub fn can_present_kms(&self) -> bool {
        // Renderer, session FD ownership and pause/resume are not wired yet.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_session_starts_disabled_and_never_claims_kms() {
        let state=NativeLifecycle::default();
        assert!(!state.seat_active());
        assert!(!state.can_present_kms());
        assert!(state.devices().is_empty());
    }
    #[test]
    fn pause_resume_is_explicit_and_remains_safe() {
        let mut state=NativeLifecycle::default();
        state.on_session(SessionEvent::ActivateSession);
        assert!(state.seat_active());
        assert!(!state.can_present_kms());
        state.on_session(SessionEvent::PauseSession);
        assert!(!state.seat_active());
        assert!(!state.can_present_kms());
        state.on_session(SessionEvent::ActivateSession);
        assert!(state.seat_active());
        assert_eq!(state.event_count(),3);
        assert!(!state.can_present_kms());
    }
    #[test]
    fn gpu_hotplug_add_changed_remove_and_replace() {
        let mut state=NativeLifecycle::default();
        state.on_device(DeviceChange::Added{id:1,path:"/dev/dri/card0".into()});
        state.on_device(DeviceChange::Added{id:2,path:"/dev/dri/card1".into()});
        state.on_device(DeviceChange::Changed{id:1});
        assert_eq!(state.devices().len(),2);
        state.on_device(DeviceChange::Added{id:1,path:"/dev/dri/card2".into()});
        assert_eq!(state.devices().get(&1),Some(&PathBuf::from("/dev/dri/card2")));
        state.on_device(DeviceChange::Removed{id:2});
        assert_eq!(state.devices().len(),1);
        assert_eq!(state.event_count(),5);
        assert!(!state.can_present_kms());
    }
    #[test]
    fn lifecycle_discards_gpu_candidates_on_seat_pause_and_hotplug() {
        let mut state=NativeLifecycle::default();
        state.on_session(SessionEvent::ActivateSession);
        assert!(state.frame_gate().seat_active());
        let initial_generation=state.frame_gate().generation();
        state.on_device(DeviceChange::Changed{id:3});
        assert!(state.frame_gate().generation()>initial_generation);
        state.on_session(SessionEvent::PauseSession);
        assert!(!state.frame_gate().seat_active());
        assert!(!state.frame_gate().actual_kms_submission_enabled());
    }
    #[test]
    fn removed_gpu_does_not_reappear_on_resume() {
        let mut state=NativeLifecycle::default();
        state.on_device(DeviceChange::Added{id:5,path:"/dev/dri/card5".into()});
        state.on_session(SessionEvent::PauseSession);
        state.on_device(DeviceChange::Removed{id:5});
        state.on_session(SessionEvent::ActivateSession);
        assert!(state.devices().is_empty());
        assert!(!state.can_present_kms());
    }
}
