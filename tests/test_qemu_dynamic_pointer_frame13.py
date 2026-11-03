"""Stage56: verify QMP resized-client screenshot checks fail closed.

Actual physical USB tablet and DRM/KMS frame are exercised inside QEMU CI,
not mocked by these unit tests.
"""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROOF = ROOT / "scripts" / "capture-qemu-dynamic-pointer.py"
INJECT = ROOT / "scripts" / "inject-qemu-dynamic-pointer.py"
BRIDGE = (ROOT / "melania-native" / "src" / "wayland_bridge.rs").read_text()
GUEST = (ROOT / "scripts" / "melania-qemu-native-guest.sh").read_text()
HOST = (ROOT / "scripts" / "run-qemu-melania-native.sh").read_text()


def proof_module():
    spec = importlib.util.spec_from_file_location("dynamic_pointer_qmp", PROOF)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ResizedClientQmpTest(unittest.TestCase):
    def test_frame13_requires_real_libinput_and_kms_ack(self):
        self.assertIn("MELANIA_NATIVE_DYNAMIC_POINTER_PROOF", BRIDGE)
        self.assertIn("MELANIA_NATIVE_DYNAMIC_POINTER_READY_AFTER_KMS12=true", BRIDGE)
        self.assertIn("MELANIA_NATIVE_DYNAMIC_POINTER_READY_AFTER_KMS12=true", INJECT.read_text())
        self.assertNotIn("wait_marker(args.serial, \"MELANIA_RUST_CONTINUOUS_FRAME_ACKED=12", INJECT.read_text())
        self.assertIn("dynamic pointer frame13 requires real xdg resize", BRIDGE)
        self.assertIn("dynamic-pointer-frame13-acked", BRIDGE)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_FRAMES=13", GUEST)
        self.assertIn("MELANIA_QEMU_DYNAMIC_POINTER_CLIENT=YES_QEMU_VM", GUEST)
        self.assertIn("MELANIA_QEMU_DYNAMIC_LIBINPUT_DRAG_FRAME13=true", HOST)
        self.assertIn("MELANIA_QEMU_DYNAMIC_POINTER_FRAME13_PIXELS=true", HOST)
        self.assertIn("input-send-event", INJECT.read_text())

    def test_pixel_validator_accepts_moved_b_and_retained_a(self):
        m = proof_module()
        ppm = bytearray(b"P6\n1280 800\n255\n" + bytes((27, 33, 58)) * (1280 * 800))
        prefix = len(b"P6\n1280 800\n255\n")
        def pixel(x, y, rgb):
            i = prefix + 3 * (y * 1280 + x)
            ppm[i:i+3] = bytes(rgb)
        pixel(200, 200, m.color(11))
        pixel(820, 380, m.blended_b(12))
        pixel(930, 420, m.blended_b(12))
        self.assertEqual(len(m.verify(bytes(ppm))), 6)

        # A ghost from the old B position is a hard failure.
        pixel(700, 250, m.blended_b(12))
        with self.assertRaisesRegex(ValueError, "B_old_pixels_cleared"):
            m.verify(bytes(ppm))

    def test_invalid_output_or_missing_moved_pixels_fails_closed(self):
        m = proof_module()
        with self.assertRaises(ValueError):
            m.verify(b"P6\n640 480\n255\n")
        data = b"P6\n1280 800\n255\n" + bytes((27, 33, 58)) * (1280 * 800)
        with self.assertRaisesRegex(ValueError, "A_retained"):
            m.verify(data)


if __name__ == "__main__":
    unittest.main()
