#!/usr/bin/env python3
"""Regression: core Melania app icons must use Fluent GTK theme aliases."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class FluentCoreAppIconsTest(unittest.TestCase):
    def test_start_and_taskbar_prefer_the_fluent_aliases(self):
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        config = (ROOT / "melania-config/src/lib.rs").read_text()
        generator = (ROOT / "tools/build_melania_fluent_themes.py").read_text()
        self.assertIn('icon_theme: "Melania-Fluent-Light".into()', config)
        for name in ("org.melaniadesktop.Files", "org.melaniadesktop.Settings"):
            self.assertIn(name, panel)
            self.assertIn(name, generator)
        self.assertIn('icon_with_fallback(icon_name,fallback)', panel)
        self.assertIn('icon_with_fallback("org.melaniadesktop.Files","system-file-manager")', panel)
        self.assertIn('icon_with_fallback("org.melaniadesktop.Settings","preferences-system")', panel)
        self.assertNotIn("start_core_icon(", panel)
        self.assertNotIn("The Files pictogram is the conventional folder", panel)


if __name__ == "__main__":
    unittest.main()
