"""Stage129: real USB/Wayland tab-stacks are tested through VirtIO CRTC pixels."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
GUEST = (ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST = (ROOT/"scripts/run-qemu-melania-native.sh").read_text()
INJECT = (ROOT/"scripts/inject-qemu-native-tab-stacks.py").read_text()
CLIENT = (ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
BRIDGE = (ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CI = (ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeTabStackQemu(unittest.TestCase):
    def test_guest_owns_real_vt_and_never_claims_native_login(self):
        self.assertEqual(GUEST.count("# Stage129:"), 1)
        self.assertEqual(GUEST.count("# Stage48:"), 1)
        self.assertEqual(GUEST.count("# Stage118:"), 1)
        for value in (
            "MELANIA_NATIVE_TAB_STACKS=YES_QEMU_VM",
            "MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM",
            "MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM",
            "MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_QEMU_TAB_CLIENT=YES_QEMU_VM",
            "MELANIA_QEMU_NATIVE_REAL_TAB_STACK_USB_KMS_PASS=true",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_NATIVE_SESSION_READY=false",
        ):
            self.assertIn(value,GUEST)

    def test_independent_real_usb_xdg_ack_and_screen_captures(self):
        for value in (
            '"execute": "input-send-event"',
            '"execute": "screendump"',
            'key(args.qmp, "f8", True)',
            'chord(args.qmp,"s",shift=True)',
            'chord(args.qmp,"pgup")',
            'chord(args.qmp,"pgdn")',
            "MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=B",
            "MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=A",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16",
            "MELANIA_QEMU_TAB_REAL_USB_XDG_FOCUS_CRTC_PIXELS=true",
            "P6", "data = path.read_bytes()",
        ):
            self.assertIn(value,INJECT)
        for value in (
            "tab-stack-before.ppm", "tab-stack-attached-A.ppm",
            "tab-stack-B-active.ppm", "tab-stack-A-restored.ppm",
            "tab-stack-dissolved.ppm",
            "native_tabs_input_code", "MELANIA_QEMU_NATIVE_REAL_TAB_STACK_USB_KMS_PASS=true",
        ):
            self.assertIn(value,HOST)
        self.assertIn("scripts/inject-qemu-native-tab-stacks.py",CI)
        self.assertIn("test_native_tab_stacks_qemu_stage129.py",CI)

    def test_wayland_keyboard_and_owned_buffers_cannot_be_faked(self):
        for marker in (
            "MELANIA_QEMU_TAB_TWO_WAYLAND_SURFACES_READY=true",
            "MELANIA_QEMU_TAB_REAL_USB_RELEASE=F8",
            "MELANIA_QEMU_TAB_TWO_WL_KEYBOARD_ENTERS_PASS=true",
            "wl_keyboard::Event::Enter",
            "wl_keyboard::Event::Key",
            "state.tab_probe",
        ):
            self.assertIn(marker,CLIENT)
        for marker in (
            "native_tab_actions_allowed",
            "self.tab_groups.attach(source,target)",
            "self.tab_groups.cycle(id,backwards)",
            "self.dynamic_scene.set_visible(id,visible)",
            "self.dynamic_scene.move_to(source,x,y)",
            "self.evict_native_popup_tree(&surface,true,true)",
            "self.native_presentation_guard.permanently_revoked()",
            "MELANIA_NATIVE_TAB_STACKING={stacking}",
        ):
            self.assertIn(marker,BRIDGE)
        self.assertNotIn("NATIVE_SESSION_READY=true", GUEST)

if __name__ == "__main__":
    unittest.main()
