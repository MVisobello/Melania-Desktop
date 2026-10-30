#!/usr/bin/env python3
"""Offline validation of the freedesktop icon theme and its integration."""
import configparser
import os
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image
from build_icon_theme import ROOT, OUT, ICONS, SIZES

class IconThemeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config=configparser.ConfigParser(interpolation=None)
        with (OUT/'index.theme').open(encoding='utf8') as f:
            cls.config.read_file(f)

    def test_theme_metadata(self):
        self.assertEqual(self.config['Icon Theme']['Name'],'Melania Aero')
        self.assertIn('hicolor',self.config['Icon Theme']['Inherits'])
        for directory in self.config['Icon Theme']['Directories'].split(','):
            self.assertIn(directory,self.config)
            self.assertTrue((OUT/directory).is_dir())

    def test_all_svg_sources_are_valid_and_safe(self):
        for f in (OUT/'scalable').rglob('*.svg'):
            xml=ET.parse(f)
            self.assertTrue(xml.getroot().tag.endswith('svg'),f)
            data=f.read_text(encoding='utf8').lower()
            self.assertNotIn('<script',data)
            self.assertNotIn('http://www.w3.org/1999/xlink',data)
            self.assertNotIn('file://',data)

    def test_all_png_sizes_and_alpha(self):
        for cat, items in ICONS.items():
            for name in items:
                for size in SIZES:
                    f=OUT/f'{size}x{size}'/cat/f'{name}.png'
                    self.assertTrue(f.is_file(),f)
                    with Image.open(f) as png:
                        self.assertEqual(png.size,(size,size))
                        self.assertIsNotNone(png.getbbox(),f)

    def test_official_mark_present_in_icon_theme(self):
        for name in ('melania','start-here','org.melaniadesktop.Files','org.melaniadesktop.Settings','org.melaniadesktop.Panel','org.melaniadesktop.Desktop'):
            self.assertTrue((OUT/'scalable/apps'/f'{name}.svg').is_file(),name)

    def test_file_manager_standard_names(self):
        source=(ROOT/'melania-files/src/main.rs').read_text()
        for name in ('folder','application-pdf','text-x-generic','x-office-document','audio-x-generic','video-x-generic'):
            self.assertIn(f'"{name}"',source)
            self.assertTrue(any((OUT/'scalable'/cat/f'{name}.svg').exists() for cat in ICONS),name)

    def test_custom_gtk_integration(self):
        for app in ('melania-files','melania-desktop-surface','melania-panel','melania-settings'):
            source=(ROOT/app/'src/main.rs').read_text()
            self.assertIn('setup_melania_icon_theme();',source)
            self.assertIn('gtk-icon-theme-name',source)

    def test_small_icons_are_simplified_not_scaled_main_svg(self):
        f=OUT/'16x16/places/folder.png'
        big=OUT/'256x256/places/folder.png'
        with Image.open(f) as a, Image.open(big) as b:
            self.assertNotEqual(a.tobytes(),b.resize((16,16)).tobytes())

    def test_installer_does_not_change_gtk_preferences_by_default(self):
        with tempfile.TemporaryDirectory() as td:
            env=dict(os.environ,XDG_DATA_HOME=td)
            installer=ROOT/'scripts/install-melania-icons.sh'
            p=subprocess.run(['bash',str(installer)],env=env,capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)
            self.assertTrue((Path(td)/'icons/Melania/index.theme').is_file())
            self.assertIn('Preferenze del desktop lasciate invariate',p.stdout)
            p=subprocess.run(['bash',str(installer),'--check'],env=env,capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
