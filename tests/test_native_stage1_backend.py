#!/usr/bin/env python3
"""Source-level rules for isolated experimental libseat/udev stage1."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
CARGO=(ROOT/"melania-native/Cargo.toml").read_text()
NATIVE=(ROOT/"melania-native/src/main.rs").read_text()
STATE=(ROOT/"melania-native/src/lib.rs").read_text()
COMP=(ROOT/"melania-compositor/src/main.rs").read_text()

class NativeStage1Contracts(unittest.TestCase):
    def test_native_feature_flags_compiled(self):
        for flag in ("backend_drm","backend_gbm","backend_libinput","backend_udev","backend_session_libseat"):
            self.assertIn(flag,CARGO)
        self.assertIn("LibSeatSession::new()?",NATIVE)
        self.assertIn('session.open(',NATIVE)
        self.assertIn('OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK',NATIVE)
        self.assertIn('DrmDeviceFd::new(DeviceFd::from(descriptor))',NATIVE)
        self.assertIn('DrmDevice::new(fd,false)',NATIVE)
        self.assertIn('device.crtcs().len()',NATIVE)

        self.assertIn("UdevBackend::new(&seat_name)?",NATIVE)
        self.assertIn("loop_events.handle().insert_source(notifier",NATIVE)
        self.assertIn("loop_events.handle().insert_source(udev",NATIVE)

    def test_no_unrequested_host_takeover(self):
        self.assertIn('MELANIA_EXPERIMENTAL_SEAT',NATIVE)
        self.assertIn('fd0.parent()!=Some(Path::new("/dev"))',NATIVE)
        self.assertIn('env::var_os("WAYLAND_DISPLAY").is_some()',NATIVE)
        self.assertIn('env::var_os("DISPLAY").is_some()',NATIVE)
        self.assertIn('NATIVE_SESSION_READY=false',NATIVE)
        self.assertNotIn('change_vt(',NATIVE)
        self.assertNotIn('Session::open(',NATIVE)
        self.assertNotIn('std::process::Command::new("systemctl")',NATIVE)

    def test_state_machine_never_misrepresents_kms(self):
        self.assertIn('SessionEvent::PauseSession',STATE)
        self.assertIn('SessionEvent::ActivateSession',STATE)
        self.assertIn('DeviceChange::Added',STATE)
        self.assertIn('DeviceChange::Removed',STATE)
        self.assertIn('pub fn can_present_kms(&self) -> bool',STATE)
        self.assertIn('false\n    }',STATE)
        self.assertIn('fn removed_gpu_does_not_reappear_on_resume()',STATE)

    def test_winit_desktop_still_intact(self):
        self.assertIn('let (mut backend, mut host) = winit::init::<GlesRenderer>()?;',COMP)
        self.assertIn('MELANIA_NESTED_SESSION',COMP)

if __name__ == "__main__":
    unittest.main()
