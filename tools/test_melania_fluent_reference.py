#!/usr/bin/env python3
"""Verify Melania's reference-faithful Fluent PNG-in-SVG artwork."""
import base64
import struct
import unittest
import xml.etree.ElementTree as ET
from melania_fluent_reference import ICON_PNG_BASE85, artwork, png_bytes
from melania_fluent_artwork import artwork as fallback


class ApprovedFluentIconTests(unittest.TestCase):
    def test_thirty_six_original_illustrations_embedded(self):
        self.assertEqual(len(ICON_PNG_BASE85), 36)
        self.assertEqual(len(set(ICON_PNG_BASE85)), 36)
        for name in ICON_PNG_BASE85:
            with self.subTest(name=name):
                png=png_bytes(name)
                self.assertEqual(png[:8],b'\x89PNG\r\n\x1a\n')
                w,h=struct.unpack('>II',png[16:24])
                self.assertEqual((w,h),(192,192))

    def test_exact_icon_name_dispatch(self):
        samples=[
            ('folder','places'),('folder-download','places'),
            ('folder-pictures','places'),('folder-music','places'),
            ('folder-videos','places'),('folder-desktop','places'),
            ('web-browser','apps'),('system-file-manager','apps'),
            ('utilities-terminal','apps'),('user-trash','places'),
            ('go-previous','actions'),('view-refresh','actions'),
            ('edit-copy','actions'),('folder-new','actions')
        ]
        for name,category in samples:
            with self.subTest(name=name):
                icon=artwork(name,category,'light',48)
                self.assertIsNotNone(icon)
                root=ET.fromstring(icon)
                self.assertTrue(root.tag.endswith('svg'))
                self.assertIn('data:image/png;base64,',icon)
                self.assertNotIn('file://',icon)

    def test_symbolic_glyphs_stay_vector_and_theme_aware(self):
        self.assertIsNone(artwork('user-home-symbolic','places','light',24))
        self.assertIsNone(artwork('audio-volume-muted-symbolic','panel','dark',24))
        self.assertNotEqual(fallback('go-previous-symbolic','actions','light',16),
                            fallback('go-previous-symbolic','actions','dark',16))


if __name__=='__main__':
    unittest.main(verbosity=2)
