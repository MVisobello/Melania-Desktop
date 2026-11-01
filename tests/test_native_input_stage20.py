#!/usr/bin/env python3
"""Stage20: native libinput+libseat event loop is real, gated and non-forwarding."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
REGISTRY=(ROOT/"melania-native/src/input_registry.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
SESSION=(ROOT/"scripts/melania-session").read_text()

class NativeInputStage20(unittest.TestCase):
    def test_real_libinput_event_source_under_libseat_not_host(self):
        stage=MAIN.split("fn experiment()",1)[1].split("/// Stage2",1)[0]
        for token in (
            "Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(",
            "session.clone().into()", "input_context.udev_assign_seat(&seat_name)?",
            "LibinputInputBackend::new(input_context.clone())",
            "insert_source(input_backend", "InputEvent::DeviceAdded",
            "InputEvent::DeviceRemoved", "DeviceCapability::Keyboard",
            "DeviceCapability::Pointer", "DeviceCapability::Touch",
            "input_context.suspend()", "input_context.resume()",
            "state.on_session(SessionEvent::PauseSession)",
            "MELANIA_NATIVE_INPUT_ENUMERATED",
            "MELANIA_NATIVE_INPUT_WAYLAND_READY=false",
        ):
            self.assertIn(token,stage)
        self.assertIn("assert_dedicated_console();",stage)
        self.assertIn("pub mod input_registry;",LIB)
        self.assertIn("input_registry.set_seat_active(false)",LIB)
        self.assertIn("input_registry.set_seat_active(true)",LIB)

    def test_seat_pause_resume_clears_all_input_device_ids(self):
        self.assertIn("self.devices.clear();",REGISTRY)
        self.assertIn("if !self.seat_active",REGISTRY)
        self.assertIn("InputRegistryError::UnknownDevice",REGISTRY)
        self.assertIn("pub fn forwards_to_wayland(&self) -> bool {false}",REGISTRY)
        self.assertIn("pub fn can_activate_native_session(&self) -> bool {false}",REGISTRY)
        self.assertIn("pause_drops_devices_and_blocks_input_until_reenumerated",REGISTRY)
        self.assertIn("removal_and_unrecognized_devices_are_rejected",REGISTRY)
        self.assertIn("event_counters_saturate_rather_than_overflow",REGISTRY)

    def test_virtual_guest_executes_real_seat_and_input_source(self):
        self.assertIn('"$binary" --experimental-seat < /dev/tty1',GUEST)
        self.assertLess(GUEST.index('"$binary" --experimental-seat'),
                        GUEST.index('"$binary" --experimental-kms-test'))
        self.assertIn("MELANIA_QEMU_NATIVE_LIBINPUT_SESSION_PASS=true",GUEST)
        self.assertIn("MELANIA_NATIVE_INPUT_ENUMERATED",HOST)
        self.assertIn("MELANIA_NATIVE_INPUT_WAYLAND_READY=false",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_LIBINPUT_SESSION_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_RESULT=FAIL",HOST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)

    def test_cannot_reach_real_device_from_graphical_host_or_login_manager(self):
        self.assertIn('if env::var_os("DISPLAY").is_some()',MAIN)
        self.assertIn('fn assert_dedicated_console()',MAIN)
        self.assertIn('NATIVE_SESSION_READY=false',MAIN)
        self.assertIn('--check-native',SESSION)
        self.assertIn('NATIVE_BACKEND_UNAVAILABLE',SESSION)
        for forbidden in ('/dev/input/event', 'libinput_context.resume().unwrap()'):
            self.assertNotIn(forbidden,MAIN)

if __name__=="__main__":
    unittest.main()
