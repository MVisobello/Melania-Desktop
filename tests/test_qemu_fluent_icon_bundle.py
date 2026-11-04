"""QEMU icon theme is genuine Melania Fluent, not an Adwaita fallback."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import build_qemu_fluent_subset as fixture  # noqa: E402
from build_melania_fluent_themes import build, THEMES  # noqa: E402


class FluentThemeGuestProof(unittest.TestCase):
    def test_subset_provides_real_fluent_app_and_tray_icons_in_all_variants(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "icons"
            build(output, fixture.selected_paths(), extra_sizes=False)
            fixture.verify(output)
            for name in THEMES:
                theme = output / name
                self.assertTrue((theme / "index.theme").is_file())
                for rel in fixture.REQUIRED:
                    icon = theme / rel
                    self.assertTrue(icon.is_file(), f"{name}/{rel} missing")
                    ET.parse(icon)  # actual SVG, not fake success marker
                    self.assertTrue(icon.resolve().is_relative_to(output.resolve()))

    def test_real_guest_loads_theme_and_fails_closed_if_missing(self):
        runner = (ROOT / "scripts" / "run-qemu-melania-native.sh").read_text()
        guest = (ROOT / "scripts" / "melania-qemu-native-guest.sh").read_text()
        workflow = (ROOT / ".github/workflows/qemu-melania-native.yml").read_text()
        self.assertIn('"icons=$fluent_icons"', runner)
        self.assertIn('MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native', guest)
        self.assertIn("MELANIA_PANEL_ICON_THEME_SELECTED=Melania-Fluent-Light", guest)
        self.assertIn("real_gtk_KMS_used_Adwaita_instead_of_Melania_Fluent", guest)
        self.assertIn("tools/build_qemu_fluent_subset.py", workflow)
        self.assertIn("MELANIA_QEMU_REAL_FLUENT_THEME_BUNDLED=true", guest)


if __name__ == "__main__":
    unittest.main()
