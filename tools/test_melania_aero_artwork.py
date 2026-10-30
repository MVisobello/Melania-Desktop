#!/usr/bin/env python3
"""Fast, dependency-free checks for the curated Melania Aero artwork."""
import unittest
import xml.etree.ElementTree as ET

from melania_aero_artwork import ACTIONS, DEVICES, FILES, FOLDERS, artwork


class CuratedAeroArtwork(unittest.TestCase):
    def test_all_curated_icons_are_valid_svg(self):
        collections = (("places", FOLDERS), ("mimetypes", FILES),
                       ("devices", DEVICES), ("actions", ACTIONS))
        for category, names in collections:
            for name in names:
                for variant in ("light", "dark"):
                    with self.subTest(category=category, name=name, variant=variant):
                        content = artwork(name, category, variant, False)
                        self.assertIsNotNone(content)
                        root = ET.fromstring(content)
                        self.assertTrue(root.tag.endswith("svg"))
                        self.assertEqual(root.attrib.get("viewBox"), "0 0 96 96")
                        self.assertNotIn("<script", content)
                        self.assertNotIn("file://", content)
                        self.assertNotIn("flat-remix", content.lower())

    def test_toolbar_contrast_and_no_tile(self):
        for name in ACTIONS:
            light = artwork(name, "actions", "light", False)
            dark = artwork(name, "actions", "dark", False)
            self.assertNotEqual(light, dark)
            self.assertIn("#edf8ff", dark)
            self.assertIn("#24557d", light)
            self.assertIn('<g fill="none"', light)
            self.assertNotIn('fill="url(#face)"', light)

    def test_original_images_remain_fallback(self):
        self.assertIsNone(artwork("firefox", "apps", "light", False))
        self.assertIsNone(artwork("folder", "places", "light", True))
        self.assertIsNone(artwork("not-a-known-icon", "devices", "light", False))

    def test_named_icons_are_not_generic_duplicates(self):
        self.assertNotEqual(artwork("folder", "places", "light", False),
                            artwork("folder-documents", "places", "light", False))
        self.assertNotEqual(artwork("application-pdf", "mimetypes", "light", False),
                            artwork("image-x-generic", "mimetypes", "light", False))


if __name__ == "__main__":
    unittest.main(verbosity=2)
