#!/usr/bin/env python3
"""Regression checks for Melania's refined Aero artwork, no extra dependencies."""
import unittest
import xml.etree.ElementTree as ET

from melania_aero_artwork import ACTIONS, DEVICES, FILES, FOLDERS
from melania_aero_details import (
    CATEGORY_MARKS, EXTRA_ACTIONS, EXTRA_DEVICES, EXTRA_FILES,
    EXTRA_FOLDERS, is_curated, refined_artwork,
)


class QualityTests(unittest.TestCase):
    def _valid(self, art, icon):
        self.assertIsNotNone(art, icon)
        root = ET.fromstring(art)
        self.assertEqual(root.attrib.get("viewBox"), "0 0 96 96")
        self.assertNotIn("<script", art.lower())
        self.assertNotIn('href="http:', art.lower())
        self.assertNotIn("file://", art.lower())

    def test_extras_full_and_compact(self):
        groups = (("places", EXTRA_FOLDERS), ("mimetypes", EXTRA_FILES),
                  ("devices", EXTRA_DEVICES), ("categories", CATEGORY_MARKS),
                  ("actions", EXTRA_ACTIONS))
        for context, names in groups:
            for name in names:
                if name.endswith("-symbolic"):
                    continue
                with self.subTest(context=context, name=name):
                    self.assertTrue(is_curated(name, context))
                    self._valid(refined_artwork(name, context, "light", False, 16), name)
                    self._valid(refined_artwork(name, context, "dark", False, 96), name)

    def test_compact_sources_are_distinct_from_detailed_sources(self):
        for name, cat in (("folder", "places"),
                          ("application-pdf", "mimetypes"),
                          ("edit-copy", "actions")):
            small = refined_artwork(name, cat, "light", False, 16)
            self._valid(small, name)
            if cat != "actions":
                from melania_aero_artwork import artwork
                large = artwork(name, cat, "light", False)
                self.assertNotEqual(small, large)

    def test_dark_status_contrast(self):
        for name in ("audio-volume-muted-symbolic",
                     "network-wireless-signal-excellent",
                     "battery-full-symbolic",
                     "bluetooth-active-symbolic"):
            for cat in ("panel", "status"):
                dark = refined_artwork(name, cat, "dark", True)
                light = refined_artwork(name, cat, "light", True)
                self._valid(dark, name)
                self._valid(light, name)
                self.assertNotEqual(dark, light)
                self.assertIn("#ecf7ff", dark)
                self.assertIn("#17416a", light)

    def test_context_app_aliases_get_actual_artwork(self):
        for name in ("computer", "drive-harddisk", "application-pdf",
                     "applications-internet"):
            self.assertTrue(is_curated(name, "apps"))
            self._valid(refined_artwork(name, "apps", "light", False, 96), name)


if __name__ == "__main__":
    unittest.main(verbosity=2)
