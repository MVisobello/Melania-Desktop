"""Stage88: only a real mapped GTK Start may receive QEMU USB keyboard input."""
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class GenuineStartKeyboardFocus(unittest.TestCase):
    def test_only_real_imported_start_surface_receives_focus(self):
        bridge = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
        self.assertIn("MELANIA_NATIVE_GTK_START_FOCUS", bridge)
        self.assertIn("self.real_gtk_pixel_kms_mode", bridge)
        self.assertIn("fn refresh_qemu_gtk_start_focus", bridge)
        self.assertIn('name=="melania-start" && role.wl_surface()==surface', bridge)
        self.assertIn("placement.rect.x>=0 && placement.rect.y>=0", bridge)
        self.assertIn("self.dynamic_scene.upsert_in_layer(", bridge)
        self.assertIn("self.refresh_qemu_gtk_start_focus(surface,true)", bridge)
        self.assertIn("keyboard.set_focus(self,Some(surface.clone())", bridge)
        self.assertIn("keyboard.set_focus(self,None", bridge)
        self.assertIn("MELANIA_NATIVE_GTK_START_REAL_KEYBOARD_FOCUS=true", bridge)
        self.assertIn("MELANIA_NATIVE_GTK_START_KEYBOARD_FOCUS_RELEASED=true", bridge)

    def test_real_gdk_entry_receives_usb_key_after_kms_start(self):
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        guest = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
        host = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
        injector = (ROOT / "scripts/inject-qemu-real-gtk-super-start.py").read_text()
        self.assertIn("MELANIA_QEMU_GTK_SEARCH_INPUT", panel)
        self.assertIn('MELANIA_QEMU_GTK_START_REAL_SEARCH_TYPED=true', panel)
        self.assertIn("query.connect_changed(move |q|", panel)
        self.assertIn("MELANIA_NATIVE_GTK_START_FOCUS=YES_QEMU_VM", guest)
        self.assertIn("MELANIA_QEMU_GTK_SEARCH_INPUT=YES_DISPOSABLE_QEMU", guest)
        self.assertIn("MELANIA_NATIVE_GTK_START_REAL_KEYBOARD_FOCUS=true", guest)
        self.assertIn("MELANIA_QEMU_GTK_START_REAL_SEARCH_TYPED=true", guest)
        self.assertIn('"type": "qcode", "data": "a"', injector)
        self.assertIn("send_usb_search_key(args.qmp)", injector)
        self.assertIn("await_serial(args.serial, THIRD, deadline)", injector)
        self.assertIn("MELANIA_QEMU_GTK_USB_SEARCH_TEXT_SENT=true", host)
        self.assertIn("MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=true", host)
        self.assertNotIn("query.set_text", injector)
        self.assertNotIn("TOGGLE_START", injector)
        for filename in ("scripts/melania-qemu-native-guest.sh",
                         "scripts/run-qemu-melania-native.sh"):
            subprocess.run(["bash", "-n", str(ROOT / filename)], check=True)
        subprocess.run(["python3", "-m", "py_compile",
                        str(ROOT / "scripts/inject-qemu-real-gtk-super-start.py")],
                       check=True)


if __name__ == "__main__":
    unittest.main()
