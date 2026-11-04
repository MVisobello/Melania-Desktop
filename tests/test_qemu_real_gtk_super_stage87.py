"""Stage87 must open real GTK4 Start from a genuine QEMU USB Super key."""
from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class RealSuperQemuStart(unittest.TestCase):
    def test_real_keyboard_event_reaches_private_gtk_panel_socket(self):
        bridge = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        guest = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
        native_cargo = (ROOT / "melania-native/Cargo.toml").read_text()
        self.assertIn('melania-control = { path = "../melania-control" }', native_cargo)
        self.assertIn('MELANIA_NATIVE_REAL_GTK_START_INPUT', bridge)
        self.assertIn('state.real_gtk_pixel_kms_mode', bridge)
        # Smithay provides an XKB keycode (evdev + 8); QEMU LEFTMETA
        # arrived as 133. Reject regressions to comparing 125 directly.
        self.assertIn("let evdev_code=code.checked_sub(8);", bridge)
        self.assertIn("matches!(evdev_code,Some(125 | 126))", bridge)
        self.assertNotIn("if code==125 || code==126", bridge)
        self.assertIn('event.state()==KeyState::Pressed', bridge)
        self.assertIn('state.qemu_start_super_down.take()==Some(code)', bridge)
        self.assertIn('state.known_window_surfaces.get(1).is_some()', bridge)
        self.assertIn('start_toggle_socket_path(Path::new(&control))', bridge)
        self.assertIn('socket.send_to(b"TOGGLE_START",&destination)', bridge)
        self.assertIn('MELANIA_NATIVE_GTK_SUPER_KEY_RELEASED=true', bridge)
        self.assertIn('MELANIA_QEMU_GTK_START_FROM_SUPER', panel)
        self.assertIn('MELANIA_QEMU_GTK_START_SUPER_IPC_OPEN={visible}', panel)
        self.assertIn('MELANIA_QEMU_GTK_START_OVERLAY_PRESENTED=true', panel)
        self.assertIn('MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM', guest)
        self.assertIn('MELANIA_QEMU_GTK_START_FROM_SUPER=YES_DISPOSABLE_QEMU', guest)
        self.assertEqual(guest.count('MELANIA_CONTROL_SOCKET=/run/melania-native-qemu/gtk-panel-control.sock'), 2)
        self.assertNotIn('MELANIA_QEMU_GTK_SHOW_START_AFTER_PAINT=YES_DISPOSABLE_QEMU', guest)
        self.assertIn('real_USB_Super_key_not_delivered_by_libinput', guest)
        self.assertIn('real_GTK_Start_not_opened_by_private_Super_IPC', guest)

    def test_usb_key_event_is_not_replaced_by_fake_ipc_or_screenshot(self):
        injector = (ROOT / "scripts/inject-qemu-real-gtk-super-start.py").read_text()
        host = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
        self.assertIn('input-send-event', injector)
        self.assertIn('"type": "key"', injector)
        self.assertIn('"type": "qcode", "data": "meta_l"', injector)
        self.assertIn('for down in (True, False)', injector)
        self.assertIn('MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE=2', injector)
        self.assertIn('MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3', injector)
        self.assertNotIn('TOGGLE_START', injector)
        self.assertNotIn('native_start.present()', injector)
        self.assertIn('inject-qemu-real-gtk-super-start.py', host)
        self.assertIn('gtk_super_input_code', host)
        self.assertIn('MELANIA_QEMU_GTK_REAL_SUPER_INPUT_PASS=true', host)
        self.assertIn('MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=true', host)
        self.assertIn('real-gtk-start-overlay-frame3.ppm', host)
        for script in ("scripts/run-qemu-melania-native.sh",
                       "scripts/melania-qemu-native-guest.sh"):
            subprocess.run(["bash", "-n", str(ROOT / script)], check=True)
        subprocess.run(["python3", "-m", "py_compile",
                        str(ROOT / "scripts/inject-qemu-real-gtk-super-start.py")],
                       check=True)


if __name__ == "__main__":
    unittest.main()
