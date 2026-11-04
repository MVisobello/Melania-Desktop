"""Fail-closed tests for Stage84's live QMP real-GTK framebuffer verifier."""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "scripts" / "capture-qemu-real-gtk-panel.py"
SPEC = importlib.util.spec_from_file_location("real_gtk_qmp_verifier", TARGET)
assert SPEC is not None and SPEC.loader is not None
verifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verifier)


def frame(with_panel: bool) -> bytes:
    w, h = verifier.WIDTH, verifier.HEIGHT
    background = bytes(verifier.BACKGROUND)
    pixels = bytearray(background * (w * h))
    if with_panel:
        for y in range(h - 52, h):
            for x in range(w):
                i = (y * w + x) * 3
                # This unit-test-only image checks pixel logic, never
                # substitutes for the required QMP screendump in CI.
                pixels[i:i+3] = bytes((30 + x % 40, 46 + y % 40, 72 + x % 60))
    return f"P6\\n{w} {h}\\n255\\n".encode() + pixels


class GtkKmsScreenshotValidation(unittest.TestCase):
    def test_rejects_background_only_even_if_correct_size(self):
        with self.assertRaisesRegex(ValueError, "does not visibly differ"):
            verifier.validate_ppm(frame(False))

    def test_requires_exact_native_qemu_dimensions(self):
        with self.assertRaisesRegex(ValueError, "1280x800"):
            verifier.validate_ppm(b"P6\\n32 32\\n255\\n" + b"\\0" * (32 * 32 * 3))

    def test_accepts_only_panel_region_with_real_variation(self):
        changed, tones = verifier.validate_ppm(frame(True))
        self.assertGreater(changed, 500)
        self.assertGreaterEqual(tones, 4)

    def test_missing_qmp_marker_cannot_claim_success(self):
        self.assertEqual(verifier.FRAME_MARKER, "MELANIA_RUST_REAL_GTK_PANEL_FRAME_VISIBLE=1")
        host_script = (ROOT / "scripts" / "run-qemu-melania-native.sh").read_text()
        self.assertIn("capture-qemu-real-gtk-panel.py", host_script)
        self.assertIn("MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=true", host_script)
        self.assertIn("real-gtk-panel-frame1.ppm", host_script)


if __name__ == "__main__":
    unittest.main()
