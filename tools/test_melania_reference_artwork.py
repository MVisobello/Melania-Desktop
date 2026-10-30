#!/usr/bin/env python3
"""Regression tests for the 30 screenshot-guided Melania Classic Aero icons."""
from __future__ import annotations
import unittest
import xml.etree.ElementTree as ET
from melania_reference_artwork import DISPLAY, ALIASES, art, artwork

class ClassicAeroIconTests(unittest.TestCase):
    def test_all_thirty_icons_have_valid_art_at_ui_sizes(self):
        self.assertEqual(len(DISPLAY), 30)
        self.assertEqual(len({name for name, _ in DISPLAY}), 30)
        for name, _ in DISPLAY:
            for size in (16, 24, 48, 96, 128, 256):
                for dark in (False, True):
                    with self.subTest(name=name, size=size, dark=dark):
                        svg = art(name, size, dark)
                        node = ET.fromstring(svg)
                        self.assertTrue(node.tag.endswith('svg'))
                        self.assertEqual(node.attrib.get('viewBox'), '0 0 96 96')
                        self.assertNotIn('<script', svg.lower())
                        self.assertNotIn('file://', svg.lower())
                        self.assertNotIn('flat-remix', svg.lower())

    def test_common_freedesktop_names_have_original_art(self):
        samples = [
            ('user-home','places'), ('folder-documents','places'),
            ('folder-pictures','places'), ('folder-music','places'),
            ('folder-videos','places'), ('folder-download','places'),
            ('user-trash','places'), ('network-server','places'),
            ('computer','apps'), ('org.melaniadesktop.Files','apps'),
            ('preferences-system','apps'), ('go-previous','actions'),
            ('go-next','actions'), ('view-refresh','actions'),
            ('edit-find','actions')
        ]
        for name, cat in samples:
            with self.subTest(name=name, category=cat):
                self.assertIsNotNone(artwork(name, cat, 'light', False, 16))

    def test_toolbar_contrast(self):
        for icon in ('go-previous','go-next','view-refresh','edit-find'):
            light = artwork(icon,'actions','light',False,16)
            dark = artwork(icon,'actions','dark',False,16)
            self.assertNotEqual(light,dark)

    def test_do_not_replace_unrelated_art(self):
        self.assertIsNone(artwork('firefox','apps','light',False,48))
        self.assertIsNone(artwork('folder','places','light',True,48))

    def test_distinct_graphic_families(self):
        self.assertGreaterEqual(len({art(key) for key, _ in DISPLAY}), 27)

if __name__ == '__main__':
    unittest.main(verbosity=2)
