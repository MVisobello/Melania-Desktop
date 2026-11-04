"""Prove QMP PPM -> PNG is lossless and refuses fabricated or partial inputs."""
from __future__ import annotations

import importlib.util
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "scripts" / "export-qemu-real-gtk-png.py"
SPEC = importlib.util.spec_from_file_location("qmp_png_encoder", TARGET)
assert SPEC is not None and SPEC.loader is not None
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class RealGtkPngEvidence(unittest.TestCase):
    def test_rejects_fake_ppm_header_and_short_pixel_payload(self):
        with tempfile.TemporaryDirectory() as temp:
            f = Path(temp) / "frame.ppm"
            f.write_bytes(b"P6\n640 480\n255\n" + b"\x00" * 16)
            with self.assertRaisesRegex(ValueError, "1280x800"):
                module.read_ppm(f)
            f.write_bytes(b"P6\n1280 800\n255\n" + b"\x00" * 16)
            with self.assertRaisesRegex(ValueError, "incorrect length"):
                module.read_ppm(f)

    def test_png_retains_first_and_last_rgb_pixel_exactly(self):
        pixels = bytearray([27, 33, 58] * (module.WIDTH * module.HEIGHT))
        pixels[:3] = b"\x01\x23\x45"
        pixels[-3:] = b"\xa1\xb2\xc3"
        png = module.encode_rgb_png(bytes(pixels))
        self.assertTrue(png.startswith(module.PNG_SIGNATURE))
        offset = len(module.PNG_SIGNATURE)
        rgb_stream = b""
        size = None
        while offset < len(png):
            n = struct.unpack(">I", png[offset:offset + 4])[0]
            tag = png[offset + 4:offset + 8]
            data = png[offset + 8:offset + 8 + n]
            crc = struct.unpack(">I", png[offset + 8 + n:offset + 12 + n])[0]
            self.assertEqual(crc, zlib.crc32(data, zlib.crc32(tag)) & 0xFFFFFFFF)
            offset += 12 + n
            if tag == b"IHDR":
                size = struct.unpack(">II", data[:8])
            if tag == b"IDAT":
                rgb_stream += zlib.decompress(data)
        self.assertEqual(size, (1280, 800))
        row = module.WIDTH * 3
        self.assertEqual(rgb_stream[0], 0)
        self.assertEqual(rgb_stream[1:4], b"\x01\x23\x45")
        self.assertEqual(rgb_stream[-3:], b"\xa1\xb2\xc3")
        self.assertEqual(len(rgb_stream), (row + 1) * module.HEIGHT)

    def test_only_verified_gtk_capture_is_exported_in_host_runner(self):
        runner = (ROOT / "scripts" / "run-qemu-melania-native.sh").read_text()
        self.assertIn("MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=true", runner)
        self.assertIn("export-qemu-real-gtk-png.py", runner)
        self.assertIn("real-gtk-panel-frame1.ppm", runner)
        self.assertIn("real-gtk-panel-frame1.png", runner)


if __name__ == "__main__":
    unittest.main()
