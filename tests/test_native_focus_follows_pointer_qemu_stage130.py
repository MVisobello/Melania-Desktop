"""Stage130: real QMP tablet motion, two Wayland Enter events and CRTC pixels."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
NATIVE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP=(ROOT/"scripts/inject-qemu-hover-focus.py").read_text()
WORKFLOW=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeHoverFocusQemuStage130(unittest.TestCase):
    def test_real_movement_produces_real_wayland_keyboard_enters(self):
        for marker in (
            "state.maybe_focus_hovered_toplevel(point)",
            "keyboard.set_focus(self,Some(surface.clone())",
            "MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=",
            'MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=YES_QEMU_VM',
        ):
            self.assertIn(marker,NATIVE+GUEST)
        for marker in (
            "wl_keyboard::Event::Enter",
            "MELANIA_QEMU_HOVER_TWO_WAYLAND_SURFACES_READY=true",
            "MELANIA_QEMU_HOVER_REAL_WL_KEYBOARD_ENTER=B",
            "MELANIA_QEMU_HOVER_REAL_WL_KEYBOARD_ENTER=A",
            "MELANIA_NATIVE_HOVER_FOCUS_TWO_REAL_KEYBOARD_ENTERS_PASS=true",
            "hover-focus-client-{index}.bgra",
        ):
            self.assertIn(marker,CLIENT)
    def test_qmp_never_sends_button_or_keyboard(self):
        self.assertIn('"execute":"input-send-event"',QMP)
        self.assertIn('"type":"abs"',QMP)
        self.assertIn('"execute":"screendump"',QMP)
        self.assertIn("verify_real_crtc_ppm",QMP)
        self.assertIn("move(args.qmp,750,280)",QMP)
        self.assertIn("move(args.qmp,250,230)",QMP)
        self.assertNotIn('"type":"btn"',QMP)
        self.assertNotIn('"type":"key"',QMP)
        self.assertIn("MELANIA_QEMU_HOVER_QMP_NO_CLICK_PASS=true",QMP)
    def test_real_guest_rollback_is_mandatory(self):
        for marker in (
            "MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM",
            "MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=YES_QEMU_VM",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=12",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_QEMU_HOVER_FOCUS_REAL_WAYLAND_PASS=true",
            "NATIVE_SESSION_READY=false",
        ):
            self.assertIn(marker,GUEST)
    def test_host_requires_real_qmp_proof(self):
        for marker in (
            "scripts/inject-qemu-hover-focus.py",
            "hover-focus-input", # no fake alternate host IPC
        ):
            if marker=="hover-focus-input":
                self.assertIn("hover_focus_input_pid",HOST)
            else:
                self.assertIn(marker,HOST)
        for marker in (
            "hover-focus-frame12.ppm",
            "MELANIA_QEMU_HOVER_QMP_NO_CLICK_PASS=true",
            "MELANIA_QEMU_HOVER_CRTC12_REAL_CLIENT_PIXELS=true",
            "MELANIA_QEMU_HOVER_FOCUS_REAL_WAYLAND_PASS=true",
        ):
            self.assertIn(marker,HOST)
        self.assertIn("python3 -m py_compile scripts/inject-qemu-hover-focus.py",WORKFLOW)
        self.assertIn("test_native_focus_follows_pointer_qemu_stage130.py",WORKFLOW)

if __name__=="__main__":
    unittest.main()
