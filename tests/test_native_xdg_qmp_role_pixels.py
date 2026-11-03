#!/usr/bin/env python3
"""Regression for the QMP source-pixel checks (test fixtures, NOT screenshots)."""
import importlib.util
from pathlib import Path
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/capture-qemu-managed-xdg-roles.py"
spec = importlib.util.spec_from_file_location("qmp_managed_role_check", SCRIPT)
assert spec and spec.loader
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def example_fixture(frame):
    # Artificial byte buffer for checking only the validator. It is never
    # published as proof of Melania's actual Wayland/KMS presentation.
    width, height = module.W, module.H
    pixels = bytearray(width * height * 3)
    for location, color in module.expected_samples(frame).values():
        x, y = location
        start = 3 * (y * width + x)
        pixels[start:start+3] = bytes(color)
    return b"P6\n1280 800\n255\n" + bytes(pixels)


class QmpManagedXdgPixelVerifierTest(unittest.TestCase):
    def test_expected_transition_pixels_are_separate_for_each_role(self):
        for frame in module.FRAMES:
            with self.subTest(frame=frame):
                self.assertGreaterEqual(len(module.verify_frame(example_fixture(frame),frame)), 5)

    def test_real_pixel_comparison_fails_on_one_wrong_sample(self):
        picture = bytearray(example_fixture(3))
        header_len = len(picture) - module.W*module.H*3
        x, y = 40, 40
        address = header_len + 3*(y*module.W+x)
        picture[address:address+3] = b"\x00\x00\x00"
        with self.assertRaisesRegex(ValueError, "A_maximized_top_left"):
            module.verify_frame(bytes(picture),3)

    def test_rejects_wrong_size_and_unrecognized_frame(self):
        with self.assertRaises(ValueError):
            module.verify_frame(b"P6\n1 1\n255\n\x00\x00\x00",3)
        with self.assertRaises(ValueError):
            module.expected_samples(4)


if __name__ == "__main__":
    unittest.main()
