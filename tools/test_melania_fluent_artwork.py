#!/usr/bin/env python3
"""Fast, dependency-free regression tests for the original Fluent vector art."""
from __future__ import annotations
import sys
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parent))
from melania_fluent_artwork import CORE, artwork

class FluentIconsTest(unittest.TestCase):
    def test_core_icons_are_valid_and_distinct(self):
        self.assertGreaterEqual(len(CORE), 45)
        kinds = set()
        for label, name, context in CORE:
            with self.subTest(name=name,context=context):
                svg = artwork(name,context,'light',96)
                ET.fromstring(svg)
                self.assertNotIn('<script',svg.lower())
                self.assertNotIn('file://',svg.lower())
                kinds.add(svg)
        self.assertGreaterEqual(len(kinds), 40)

    def test_toolbar_appearance_changes_by_theme(self):
        for name in ('go-previous','go-next','edit-find','view-refresh','edit-copy'):
            self.assertNotEqual(artwork(name,'actions','light',16),
                                artwork(name,'actions','dark',16))

    def test_all_essential_sizes_render_as_valid_svg(self):
        for name,cat in (('folder','places'),('folder-download','places'),
                         ('folder-pictures','places'),('application-pdf','mimetypes'),
                         ('user-home','places'),('network-wireless-signal-excellent','panel')):
            for variant in ('light','dark'):
                for size in (16,24,48,96,256):
                    ET.fromstring(artwork(name,cat,variant,size))
if __name__=='__main__':
    unittest.main(verbosity=2)
