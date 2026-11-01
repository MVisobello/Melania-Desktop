#!/usr/bin/env python3
"""XFCE 4.20 category coverage + live XDG settings contract tests."""
from pathlib import Path
import re
import unittest

ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"melania-settings/src/main.rs").read_text()
CATALOG=(ROOT/"melania-settings/src/xfce_catalog.rs").read_text()
PAGES=(ROOT/"melania-settings/src/xfce_pages.rs").read_text()
MIME=(ROOT/"melania-settings/src/xfce_defaults.rs").read_text()
EXPECTED = {
    "accessibility","appearance","desktop","display","file-manager","keyboard",
    "mouse","panel","power","preferred","mime","removable","session",
    "settings-editor","window-manager","wm-tweaks","workspaces","notifications",
    "screensaver","printing","color","sound","network","bluetooth",
}

class XfceSettingsParity(unittest.TestCase):
    def test_complete_documented_category_inventory(self):
        actual=set(re.findall(r'XfceSection \{ slug: "([^"]+)"',CATALOG))
        self.assertEqual(actual,EXPECTED)
        self.assertIn("XFCE_SECTIONS",CATALOG)
        self.assertIn("pub fn has_page(page:&str)->bool",CATALOG)

    def test_all_catalog_pages_have_reachable_navigation(self):
        self.assertIn("mod xfce_catalog;",SOURCE)
        self.assertIn("mod xfce_defaults;",SOURCE)
        self.assertIn("mod xfce_pages;",SOURCE)
        self.assertIn("xfce_pages::add_pages(&stack);",SOURCE)
        self.assertIn('("Impostazioni complete XFCE", "xfce"',SOURCE)
        self.assertIn('xfce_pages::is_known_page(page)',SOURCE)
        self.assertIn('for spec in XFCE_SECTIONS {',PAGES)
        self.assertIn('stack.add_titled(&body,Some(&target),spec.title);',PAGES)
        self.assertIn('gnome_pages::linked_row(',PAGES)
        self.assertIn('"Non ancora operativo"',PAGES)
        self.assertIn('xfce_defaults::mime_page()',PAGES)
        self.assertIn('xfce_defaults::preferred_apps_page()',PAGES)

    def test_mime_settings_are_real_gio_not_fake_buttons_or_shell(self):
        self.assertIn("gio::AppInfo::all_for_type(mime)",MIME)
        self.assertIn("gio::AppInfo::default_for_type(mime",MIME)
        self.assertIn("app.set_as_default_for_type(&kind)",MIME)
        self.assertIn('app.set_as_default_for_type("x-scheme-handler/https")',MIME)
        self.assertIn("save.connect_clicked(move |_|",MIME)
        self.assertIn("fn is_valid_type(mime:&str)->bool",MIME)
        self.assertNotIn("Command::new(",MIME)
        self.assertIn("condivise",MIME)

if __name__=="__main__":
    unittest.main(verbosity=2)
