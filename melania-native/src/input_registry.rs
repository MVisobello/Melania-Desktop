//! Stage 20: native libinput device lifecycle accounting, without input delivery.
//!
//! Only the explicitly gated TTY experiment may attach a real libinput
//! event source. No keyboard contents, coordinates, or user actions are logged.
//! Device IDs are invalidated on both seat pause and resume. This registry
//! does not synthesize Wayland input, inject keys into the host or run a session.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputCapabilities {
    pub keyboard: bool,
    pub pointer: bool,
    pub touch: bool,
}

impl InputCapabilities {
    pub fn any(&self) -> bool { self.keyboard || self.pointer || self.touch }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRegistryError {
    InactiveSeat,
    UnknownDevice,
    NoCapabilities,
}

#[derive(Debug, Default)]
pub struct NativeInputRegistry {
    seat_active: bool,
    generation: u64,
    devices: BTreeMap<String, InputCapabilities>,
    accepted_events: u64,
    rejected_events: u64,
}

impl NativeInputRegistry {
    pub fn seat_active(&self) -> bool { self.seat_active }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn device_count(&self) -> usize { self.devices.len() }
    pub fn accepted_events(&self) -> u64 { self.accepted_events }
    pub fn rejected_events(&self) -> u64 { self.rejected_events }

    /// Release stale libinput identities on pause AND resume. Real device
    /// enumeration must happen again after libinput's resume, not be inferred
    /// from IDs previously observed by this process.
    pub fn set_seat_active(&mut self, active: bool) {
        self.seat_active = active;
        self.generation = self.generation.wrapping_add(1);
        self.devices.clear();
    }

    pub fn device_added(
        &mut self,
        id: String,
        caps: InputCapabilities,
    ) -> Result<(), InputRegistryError> {
        if !self.seat_active {
            return Err(InputRegistryError::InactiveSeat);
        }
        if id.is_empty() || !caps.any() {
            return Err(InputRegistryError::NoCapabilities);
        }
        self.devices.insert(id,caps);
        Ok(())
    }

    pub fn device_removed(&mut self, id: &str) {
        self.devices.remove(id);
    }

    /// Count only events from currently registered devices on the active
    /// seat. No raw key values or coordinates are stored.
    pub fn event(&mut self, id: &str) -> Result<(), InputRegistryError> {
        if !self.seat_active {
            self.rejected_events = self.rejected_events.saturating_add(1);
            return Err(InputRegistryError::InactiveSeat);
        }
        if !self.devices.contains_key(id) {
            self.rejected_events = self.rejected_events.saturating_add(1);
            return Err(InputRegistryError::UnknownDevice);
        }
        self.accepted_events = self.accepted_events.saturating_add(1);
        Ok(())
    }

    pub fn counts(&self) -> (usize,usize,usize) {
        self.devices.values().fold((0,0,0),|(k,p,t),c|
            (k+usize::from(c.keyboard),
             p+usize::from(c.pointer),
             t+usize::from(c.touch)))
    }

    pub fn forwards_to_wayland(&self) -> bool {false}
    pub fn can_activate_native_session(&self) -> bool {false}
}

#[cfg(test)]
mod tests {
    use super::*;
    const KEYBOARD:InputCapabilities=InputCapabilities{
        keyboard:true,pointer:false,touch:false
    };
    const MOUSE:InputCapabilities=InputCapabilities{
        keyboard:false,pointer:true,touch:false
    };
    #[test]
    fn enumerates_multiple_real_capability_classes_without_accepting_host_input() {
        let mut r=NativeInputRegistry::default();
        assert_eq!(r.device_added("kbd".into(),KEYBOARD),Err(InputRegistryError::InactiveSeat));
        r.set_seat_active(true);
        r.device_added("kbd".into(),KEYBOARD).unwrap();
        r.device_added("mouse".into(),MOUSE).unwrap();
        assert_eq!(r.counts(),(1,1,0));
        r.event("kbd").unwrap();
        r.event("mouse").unwrap();
        assert_eq!(r.accepted_events(),2);
        assert!(!r.forwards_to_wayland());
        assert!(!r.can_activate_native_session());
    }
    #[test]
    fn pause_drops_devices_and_blocks_input_until_reenumerated() {
        let mut r=NativeInputRegistry::default();
        r.set_seat_active(true);
        r.device_added("kbd".into(),KEYBOARD).unwrap();
        let old_epoch=r.generation();
        r.set_seat_active(false);
        assert_eq!(r.device_count(),0);
        assert_eq!(r.event("kbd"),Err(InputRegistryError::InactiveSeat));
        r.set_seat_active(true);
        assert_ne!(old_epoch,r.generation());
        assert_eq!(r.event("kbd"),Err(InputRegistryError::UnknownDevice));
        r.device_added("kbd".into(),KEYBOARD).unwrap();
        assert!(r.event("kbd").is_ok());
        assert_eq!(r.rejected_events(),2);
    }
    #[test]
    fn removal_and_unrecognized_devices_are_rejected() {
        let mut r=NativeInputRegistry::default();
        r.set_seat_active(true);
        assert_eq!(r.device_added("".into(),KEYBOARD),
            Err(InputRegistryError::NoCapabilities));
        assert_eq!(r.device_added("joystick".into(),InputCapabilities{
            keyboard:false,pointer:false,touch:false,
        }),Err(InputRegistryError::NoCapabilities));
        r.device_added("mouse".into(),MOUSE).unwrap();
        r.device_removed("mouse");
        assert_eq!(r.event("mouse"),Err(InputRegistryError::UnknownDevice));
        assert_eq!(r.device_count(),0);
    }
    #[test]
    fn touch_and_composite_devices_are_counted_without_logging_payloads() {
        let mut r=NativeInputRegistry::default();
        r.set_seat_active(true);
        r.device_added("combo".into(),InputCapabilities{
            keyboard:true,pointer:true,touch:true,
        }).unwrap();
        assert_eq!(r.counts(),(1,1,1));
        r.set_seat_active(true);
        assert_eq!(r.device_count(),0);
    }
    #[test]
    fn event_counters_saturate_rather_than_overflow() {
        let mut r=NativeInputRegistry::default();
        r.rejected_events=u64::MAX;
        assert_eq!(r.event("unknown"),Err(InputRegistryError::InactiveSeat));
        assert_eq!(r.rejected_events(),u64::MAX);
    }
}
