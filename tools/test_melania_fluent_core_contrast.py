#!/usr/bin/env python3
"""Real light Start uses visible Fluent vectors, never white PNG cutouts."""
import unittest
from xml.etree import ElementTree as ET

from build_melania_fluent_themes import core_app_icon


class FluentCoreAppContrastTests(unittest.TestCase):
    def test_files_and_settings_are_colorful_svg_in_all_theme_variants(self):
        names=("org.melaniadesktop.Files", "org.melaniadesktop.Settings")
        for name in names:
            for variant in ("light","dark"):
                for size in (16,24,48,96):
                    with self.subTest(name=name,variant=variant,size=size):
                        svg=core_app_icon(name,"apps",variant,size)
                        self.assertIsNotNone(svg)
                        root=ET.fromstring(svg)
                        self.assertTrue(root.tag.endswith("svg"))
                        self.assertNotIn("data:image/png",svg)
                        self.assertNotIn("<image",svg)
                        self.assertIn("#",svg)
                        self.assertIn('fill=',svg)
        self.assertNotEqual(core_app_icon(names[0],"apps","light",24),
                            core_app_icon(names[1],"apps","light",24))

    def test_symbolic_tray_and_regular_third_party_icons_are_unmodified(self):
        self.assertIsNone(core_app_icon("melania-tray-wifi","status","light",24))
        self.assertIsNone(core_app_icon("firefox","apps","light",48))
        self.assertIsNone(core_app_icon("org.melaniadesktop.Files","places","light",24))


if __name__=="__main__":
    unittest.main()
