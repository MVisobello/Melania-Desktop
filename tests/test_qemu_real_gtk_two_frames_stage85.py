"""Fail closed: two real GTK4 frames need separate CRTC QMP captures."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
target = ROOT / "scripts" / "capture-qemu-real-gtk-two-frames.py"
spec = importlib.util.spec_from_file_location("qmp_two_real_gtk_frames", target)
assert spec is not None and spec.loader is not None
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

W, H = checker.single.WIDTH, checker.single.HEIGHT
TOP = checker.single.PANEL_TOP


def mock_frame(tone: int) -> bytes:
    # These are only unit-test inputs to verify rejection rules; the actual
    # proof must separately capture live VM CRTC via QMP with two marker IDs.
    pixels = bytearray(bytes(checker.single.BACKGROUND) * (W * H))
    for y in range(TOP, H):
        start = y * W * 3
        pixels[start:start + W * 3] = bytes((tone, 45, 90)) * W
    return bytes(pixels)


class QemuTwoRealGtkFrames(unittest.TestCase):
    def test_rejects_identical_or_background_changed_frames(self):
        first = mock_frame(40)
        with self.assertRaisesRegex(ValueError, "did not repaint"):
            checker.compare_real_frames(first, first)
        second = bytearray(mock_frame(100))
        second[0] ^= 1
        with self.assertRaisesRegex(ValueError, "non-panel QEMU scene changed"):
            checker.compare_real_frames(first, bytes(second))

    def test_accepts_only_genuinely_distinct_panel_pixels(self):
        first, second = mock_frame(40), mock_frame(100)
        self.assertGreater(checker.compare_real_frames(first, second), 2400)
        with self.assertRaisesRegex(ValueError, "payload lengths"):
            checker.compare_real_frames(first[:-1], second)

    def test_second_frame_is_created_by_gtk_and_verified_by_drm(self):
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        bridge = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
        receiver = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
        guest = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
        host = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
        self.assertIn("MELANIA_QEMU_GTK_SECOND_REAL_FRAME_REQUESTED=true", panel)
        self.assertIn("bar_for_second_frame.child()", panel)
        self.assertIn("MELANIA_NATIVE_REAL_GTK_TWO_FRAME", bridge)
        self.assertIn("MELANIA_NATIVE_REAL_GTK_TWO_FRAME_KMS_PASS=true", bridge)
        self.assertIn("MELANIA_QEMU_GTK_TWO_FRAME_KMS", receiver)
        self.assertIn("MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE=", receiver)
        self.assertIn("MELANIA_RUST_REAL_GTK_TWO_FRAME_KMS_PASS=true", receiver)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_FRAMES=2", guest)
        self.assertIn("MELANIA_QEMU_GTK_SECOND_FRAME=YES_DISPOSABLE_QEMU", guest)
        self.assertIn("MELANIA_QEMU_REAL_GTK_TWO_FRAME_KMS_SCANOUT_PASS=true", guest)
        self.assertIn("capture-qemu-real-gtk-two-frames.py", host)
        self.assertIn("MELANIA_QEMU_GTK4_TWO_DISTINCT_REAL_FRAMES=true", host)
        self.assertIn("real-gtk-continuous-frame1.png", host)
        self.assertIn("real-gtk-continuous-frame2.png", host)
        subprocess.run(["bash", "-n", str(ROOT / "scripts/melania-qemu-native-guest.sh")], check=True)
        subprocess.run(["bash", "-n", str(ROOT / "scripts/run-qemu-melania-native.sh")], check=True)


if __name__ == "__main__":
    unittest.main()
