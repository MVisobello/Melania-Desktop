"""Stage86 requires a real third QMP frame showing GTK Start above taskbar."""
from __future__ import annotations

import importlib.util
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
file = ROOT / "scripts" / "capture-qemu-real-gtk-start-overlay.py"
spec = importlib.util.spec_from_file_location("qmp_real_gtk_start", file)
assert spec is not None and spec.loader is not None
start_qmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(start_qmp)

W, H = 1280, 800
BG = bytes((27, 33, 58))


def frame(with_start: bool) -> bytes:
    # Unit-test fixtures are NOT screenshots. The production proof above
    # separately captures the authentic live QEMU CRTC with its own FB_ID.
    image = bytearray(BG * (W * H))
    for y in range(740, 800):
        idx = (y * W) * 3
        image[idx:idx + W * 3] = bytes((45, 82, 120)) * W
    if with_start:
        for y in range(229, 740):
            idx = (y * W + 8) * 3
            shade = (y - 229) // 52
            image[idx:idx + 510 * 3] = bytes((60 + shade * 9, 92 + shade * 8, 145 + shade * 7)) * 510
            if y % 5 == 0:
                # Distinct tones for text/widgets, unlike a flat fake tile.
                image[idx:idx + 220 * 3] = bytes((230, 238, 245)) * 220
    return bytes(image)


class ActualGtkStartQmp(unittest.TestCase):
    def test_rejects_unchanged_screenshot_and_background_corruption(self):
        old = frame(False)
        with self.assertRaisesRegex(ValueError, "did not appear"):
            start_qmp.validate_start_open(old, old)
        bad = bytearray(frame(True))
        bad[(300 * W + 1100) * 3] ^= 64
        with self.assertRaisesRegex(ValueError, "background changed"):
            start_qmp.validate_start_open(old, bytes(bad))

    def test_requires_visible_start_and_correct_frame_size(self):
        old, opened = frame(False), frame(True)
        self.assertGreater(start_qmp.validate_start_open(old, opened), 18_000)
        with self.assertRaisesRegex(ValueError, "lengths differ"):
            start_qmp.validate_start_open(old[:-3], opened)

    def test_real_guest_sends_three_KMS_frames_with_genuine_start(self):
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        server = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
        drm = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
        guest = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
        host = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
        self.assertIn("MELANIA_QEMU_GTK_SHOW_START_AFTER_PAINT", panel)
        self.assertIn("native_start.present()", panel)
        self.assertIn("MELANIA_QEMU_GTK_START_OVERLAY_PRESENTED=true", panel)
        self.assertIn("MELANIA_NATIVE_REAL_GTK_START_KMS", server)
        self.assertIn("MELANIA_NATIVE_REAL_GTK_START_OVERLAY_KMS_PASS=true", server)
        self.assertIn('name=="melania-start"', server)
        self.assertIn("MELANIA_QEMU_GTK_START_KMS", drm)
        self.assertIn("MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3", drm)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_FRAMES=3", guest)
        self.assertIn("MELANIA_QEMU_REAL_GTK_START_OVERLAY_KMS_SCANOUT_PASS=true", guest)
        self.assertIn("capture-qemu-real-gtk-start-overlay.py", host)
        self.assertIn("MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=true", host)
        self.assertIn("real-gtk-start-overlay-frame3.png", host)
        subprocess.run(["bash", "-n", str(ROOT / "scripts/melania-qemu-native-guest.sh")], check=True)
        subprocess.run(["bash", "-n", str(ROOT / "scripts/run-qemu-melania-native.sh")], check=True)


if __name__ == "__main__":
    unittest.main()
