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

    def test_all_operational_pages_are_reachable_from_the_sidebar(self):
        self.assertIn("mod xfce_catalog;", SOURCE)
        self.assertIn("mod xfce_pages;", SOURCE)
        self.assertIn("xfce_pages::add_pages(&stack);", SOURCE)
        self.assertIn("xfce_pages::canonical_page(page)", SOURCE)
        self.assertIn("xfce_pages::is_known_page(page)", SOURCE)
        self.assertNotIn('("Impostazioni complete XFCE", "xfce"', SOURCE)

        # The same registry drives sidebar construction and CLI validation.
        match = re.search(r'const SETTINGS_NAVIGATION:.*?= \\[(.*?)\\];', SOURCE, re.S)
        self.assertIsNotNone(match)
        navigation = re.findall(r'^\\s*\\("[^"]+",\\s*"([^"]+)",\\s*"[^"]+"\\),',
                                match.group(1), re.M)
        self.assertEqual(len(navigation), 25)
        self.assertEqual(len(set(navigation)), 25)
        self.assertIn("for &(name, target, icon_name) in SETTINGS_NAVIGATION.iter()", SOURCE)
        self.assertIn("SETTINGS_NAVIGATION.iter().any(", SOURCE)

        # Every Stack page is directly in the categorized Settings sidebar:
        # no hidden XFCE-only tabs and no unimplemented placeholder pages.
        main = (ROOT / "melania-settings/src/main.rs").read_text()
        gnome = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        all_sources = main + gnome + PAGES
        registered = set(re.findall(
            r'stack\\.add_titled\\([^;]*?Some\\("([^"]+)"\\)', all_sources, re.S
        ))
        self.assertEqual(set(navigation), registered)
        self.assertNotIn("Non ancora operativo", PAGES)
        self.assertNotIn('Some("xfce")', PAGES)
        for target in ("xfce-desktop", "xfce-panel", "xfce-session",
                       "xfce-preferred", "xfce-mime"):
            self.assertIn(target, navigation)
            self.assertIn(f'Some("{target}")', PAGES)

        # Preserve old CLI URLs while steering duplicates to a real page.
        self.assertIn('page.strip_prefix("xfce-") == Some(spec.slug)', PAGES)
        self.assertIn("spec.melania_target", PAGES)
        self.assertIn('"xfce" => "system"', PAGES)
        self.assertIn('xfce_defaults::mime_page()', PAGES)
        self.assertIn('crate::xfce_autostart::session_page()', PAGES)
        autostart = (ROOT / "melania-settings/src/xfce_autostart.rs").read_text()
        self.assertIn("set_enabled_in(dir:&Path", autostart)
        self.assertIn("Hidden=", autostart)
        self.assertIn("create_new(true).mode(0o600)", autostart)
        self.assertIn("fs::rename(&temp,&path)", autostart)
        self.assertIn("xfce_defaults::preferred_apps_page()", PAGES)

    def test_page_titles_go_directly_to_controls_without_introductions(self):
        main = (ROOT / "melania-settings/src/main.rs").read_text()
        gnome = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        panel = (ROOT / "melania-settings/src/xfce_panel.rs").read_text()
        desktop = (ROOT / "melania-settings/src/xfce_desktop.rs").read_text()
        defaults = (ROOT / "melania-settings/src/xfce_defaults.rs").read_text()
        autostart = (ROOT / "melania-settings/src/xfce_autostart.rs").read_text()
        self.assertNotIn('introduction.append(&text(description', main)
        self.assertNotIn('words.append(&label(subtitle', gnome)
        for implementation in (panel, desktop, defaults, autostart):
            self.assertNotIn('come in XFCE', implementation)
            self.assertNotIn('"settings-subtitle"', implementation)

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
