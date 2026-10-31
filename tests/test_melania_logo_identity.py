#!/usr/bin/env python3
"""Checks that the original, author-approved Melania mark is reused, not redrawn."""
from pathlib import Path
import sys
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
from build_melania_fluent_themes import original_melania_logo


class MelaniaIdentityContracts(unittest.TestCase):
    def test_canonical_logo_is_valid_svg_and_contains_crescent_and_dot(self):
        source = ROOT / "branding" / "icons" / "melania-symbol.svg"
        svg = original_melania_logo()
        self.assertEqual(svg, source.read_text(encoding="utf8"))
        self.assertEqual(ET.fromstring(svg).tag, "{http://www.w3.org/2000/svg}svg")
        self.assertEqual(svg.count("<path"), 2)

    def test_fluent_builder_never_regenerates_original_logo(self):
        builder = (ROOT / "tools" / "build_melania_fluent_themes.py").read_text()
        self.assertIn("category == 'apps' and name == 'melania'", builder)
        self.assertIn("original_melania_logo()", builder)
        self.assertIn("Melania-Fluent-Light", builder)
        self.assertIn("Melania-Fluent-Dark", builder)

    def test_nested_wayland_clients_cannot_request_host_shutdown(self):
        compositor = (ROOT / "melania-compositor" / "src" / "main.rs").read_text()
        session = (ROOT / "melania-panel" / "src" / "session_actions.rs").read_text()
        self.assertEqual(compositor.count('.env("MELANIA_NESTED_SESSION", "1")'), 2)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS', session)
        self.assertIn('MELANIA_NESTED_SESSION', session)
        self.assertIn('MELANIA_SMOKE_OPEN_START', session)
        self.assertIn('availability(action)?;', session)
        self.assertIn('ControlCommand::Quit', session)


if __name__ == "__main__":
    unittest.main(verbosity=2)
