"""Stage116: full QEMU USB keyboard, Wayland focus, and kernel CRTC contract."""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
NATIVE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
INJECT=(ROOT/"scripts/inject-qemu-alt-tab.py").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class RealUsbAltTabQemuStage116(unittest.TestCase):
    def test_real_native_keyboard_and_wayland_surface_focus(self):
        for marker in (
            "state.keyboard_window_cycle.event(",
            "code.checked_sub(8)",
            "event.state()==KeyState::Pressed",
            "state.dynamic_scene.focusable_normal_keys_top_first()",
            "state.known_window_surfaces.get(key).cloned()",
            "keyboard.set_focus(state,Some(surface.clone())",
            "state.submit_continuous_frame()",
            "MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=",
            "MELANIA_NATIVE_KEYBOARD_ALT_F4_CLOSE_REQUESTED=",
            "top.send_close()",
        ):
            self.assertIn(marker,NATIVE)
        for marker in (
            '"wl_seat"=>state.seat=Some',
            "seat.as_ref().ok_or",
            "wl_keyboard::Event::Enter",
            "s.second_surface.as_ref()==Some(&surface)",
            "s.first_surface.as_ref()==Some(&surface)",
            "MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=B",
            "MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=A",
            "MELANIA_QEMU_ALT_TAB_TWO_WAYLAND_SURFACES_READY=true",
            "MELANIA_NATIVE_ALT_TAB_TWO_REAL_KEYBOARD_ENTERS_PASS=true",
            "MELANIA_QEMU_ALT_F4_REAL_XDG_CLOSE_RECEIVED=A",
            "MELANIA_NATIVE_ALT_F4_REAL_XDG_CLOSE_PASS=true",
        ):
            self.assertIn(marker,CLIENT)

    def test_kernel_ack_and_real_crtc_pixels_required(self):
        for marker in (
            "MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM",
            "MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=2",
            "MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=1",
            "MELANIA_NATIVE_KEYBOARD_ALT_F4_CLOSE_REQUESTED=1",
            "MELANIA_NATIVE_ALT_F4_REAL_XDG_CLOSE_PASS=true",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_QEMU_REAL_USB_ALT_TAB_WAYLAND_KMS_PASS=true",
            "MELANIA_NATIVE_SESSION_READY=false",
        ):
            self.assertIn(marker,GUEST)
        for marker in (
            '"execute":"input-send-event"',
            '"type":"key"',
            'key(args.qmp,"alt",True)',
            'key(args.qmp,"tab",True)',
            'key(args.qmp,"f4",True)',
            "MELANIA_QEMU_ALT_F4_REAL_WAYLAND_CLOSE=true",
            "MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=B",
            "MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=A",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",
            '"execute":"screendump"',
            "retained_client_A_frame11",
            "retained_client_B_frame12",
        ):
            self.assertIn(marker,INJECT)
        for marker in (
            "scripts/inject-qemu-alt-tab.py",
            "alt-tab-frame13.ppm",
            "MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER_A_B=true",
            "MELANIA_QEMU_ALT_TAB_REAL_KERNEL_CRTC_FRAME13_PIXELS=true",
            "MELANIA_QEMU_REAL_USB_ALT_TAB_WAYLAND_KMS_PASS=true",
        ):
            self.assertIn(marker,HOST)
        self.assertIn("python3 -m py_compile scripts/inject-qemu-alt-tab.py",CI)
        self.assertIn("test_native_alt_tab_qemu_stage116.py",CI)

if __name__=="__main__":
    unittest.main()
