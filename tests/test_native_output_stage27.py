#!/usr/bin/env python3
"""Regression checks for Stage 27 native wl_output and true nested UI capture."""
from pathlib import Path
import importlib.util
import tempfile
import unittest
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SERVER = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
CLIENT = (ROOT / "melania-native/src/bin/qemu-client-window.rs").read_text()
HOST = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
NESTED = (ROOT / "scripts/smoke-full-desktop-wayland.sh").read_text()
WORKFLOW = (ROOT / ".github/workflows/wayland-smoke.yml").read_text()

spec = importlib.util.spec_from_file_location(
    "real_desktop", ROOT / "scripts/check-real-desktop-preview.py"
)
preview = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preview)


class NativeOutputStage27(unittest.TestCase):
    def test_native_advertises_output_but_does_not_claim_kms(self):
        for fragment in (
            "OutputManagerState::new_with_xdg_output::<BridgeState>",
            "delegate_output!(BridgeState)",
            'Output::new("MELANIA-QEMU-1"',
            "output.create_global::<BridgeState>",
            "output.change_current_state(Some(virtual_mode)",
            "output.set_preferred(virtual_mode)",
            "MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800",
            "MELANIA_NATIVE_WAYLAND_RENDER_READY=false",
            "MELANIA_NATIVE_LIVE_KMS_RENDERING=false",
        ):
            self.assertIn(fragment, SERVER)

    def test_actual_client_requires_wl_output_events_not_a_server_log(self):
        for fragment in (
            "wl_output::WlOutput",
            '"wl_output"=>state.output=Some(registry.bind',
            "wl_output::Event::Mode{width,height,refresh,..}",
            "if width==1280 && height==800 && refresh>0",
            "if state.output.is_none() || !state.output_mode_received",
            "MELANIA_NATIVE_CLIENT_WL_OUTPUT_MODE=1280x800",
        ):
            self.assertIn(fragment, CLIENT)
        for fragment in (
            "MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800",
            "MELANIA_NATIVE_CLIENT_WL_OUTPUT_MODE=1280x800",
            "MELANIA_QEMU_NATIVE_WL_OUTPUT_PASS=true",
            "MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",
        ):
            self.assertIn(fragment, HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_WL_OUTPUT_PASS=true", GUEST)

    def test_real_ui_artifacts_are_not_generated_ui(self):
        self.assertIn("scripts/check-real-desktop-preview.py", NESTED)
        self.assertIn("reports/melania-real-desktop-preview.png", NESTED)
        self.assertIn("reports/melania-real-desktop-preview.png", WORKFLOW)
        self.assertIn("real_nested_wayland_xvfb", (ROOT / "scripts/check-real-desktop-preview.py").read_text())

    def test_real_capture_copy_is_byte_identical(self):
        with tempfile.TemporaryDirectory() as d:
            temp = Path(d)
            original = temp / "desktop.png"
            search = temp / "search.png"
            windows = temp / "windows.log"
            windows.write_text("1\tMelania Files\n2\tMelania Settings\n")
            img = Image.new("RGB", (1280, 800), (27, 33, 58))
            draw = ImageDraw.Draw(img)
            draw.rectangle((100, 100, 1100, 690), fill=(235, 235, 235))
            for i in range(20):
                draw.rectangle((100 + i * 50, 500, 149 + i * 50, 550),
                               fill=(10 + 7 * i, 30, 100))
            img.save(original)
            draw.rectangle((310, 400, 600, 460), fill=(60, 120, 220))
            img.save(search)
            manifest = preview.verify(original, search, windows)
            self.assertTrue(manifest["start_search_visual_change"])
            self.assertFalse(manifest["native_kms_desktop"])
            self.assertGreater(manifest["distinct_sample_colors"], 8)
            windows.write_text("3\tUnrelated\n")
            with self.assertRaises(ValueError):
                preview.verify(original, search, windows)
            windows.write_text("1\tMelania Files\n2\tMelania Settings\n")
            with self.assertRaises(ValueError):
                preview.verify(original, original, windows)


if __name__ == "__main__":
    unittest.main()
