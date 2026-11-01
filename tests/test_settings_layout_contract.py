"""Melania GNOME Settings and Windows 7 chrome integration regression tests."""
import sys
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]

class SettingsUIContract(unittest.TestCase):
    def test_gtk_navigation_and_system_categories(self):
        main = (ROOT / "melania-settings/src/main.rs").read_text()
        pages = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        self.assertIn("mod gnome_pages;", main)
        self.assertIn("gnome_pages::overview(&stack)", main)
        self.assertIn("gnome_pages::add_pages(&stack)", main)
        self.assertIn("stack.set_visible_child_name(&target)", pages)
        for name in ("system","network","bluetooth","appearance","notifications",
                     "applications","account","datetime","power","audio","devices","privacy",
                     "region","accessibility","about","desktop","workspaces","shortcuts",
                     "display","storage"):
            self.assertIn(f'"{name}"', main)
        self.assertIn('RefCell::new("system".into())', main)

    def test_live_navigation_audit_covers_every_registered_settings_page(self):
        main=(ROOT / "melania-settings/src/main.rs").read_text()
        pages=(ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        audit=(ROOT / "scripts/audit-settings-nav-gtk.sh").read_text()
        workflow=(ROOT / ".github/workflows/settings-visual.yml").read_text()
        for marker in (
            'stack.pages().n_items()',
            'stack.child_by_name(target).is_some()',
            'MELANIA_SETTINGS_PAGES_OK={pages}',
            'MELANIA_SETTINGS_ACTIVE_PAGE={page}',
            'MELANIA_SETTINGS_AUDIT',
        ):
            self.assertIn(marker, main)
        for target in ("xfce-preferred", "xfce-mime", "xfce-session"):
            self.assertIn(f'linked_row(stack, "{target}"', pages)
        self.assertIn('target/debug/melania-settings "$page"', audit)
        self.assertIn('settings-audit-$page.png', audit)
        self.assertIn("bash scripts/audit-settings-nav-gtk.sh", workflow)

    def test_independent_settings_pages_do_not_overwrite_each_other(self):
        source=(ROOT / "melania-settings/src/main.rs").read_text()
        self.assertIn("fn save_edit(", source)
        self.assertIn("let mut latest = melania_config::load()", source)
        self.assertIn("edit(&mut latest);", source)
        self.assertIn("*shared.borrow_mut() = latest.clone();", source)
        # All edits from the main Settings window must rebase on disk,
        # because Panel and Desktop edit that same file independently.
        self.assertEqual(source.count("melania_config::save("), 1)
        for callback in ("prefs", "prefs_for_theme", "prefs_for_icons",
                         "preferences", "workspace_preferences"):
            self.assertIn(f"save_edit(&{callback},", source)
        for path in ("xfce_panel.rs","xfce_desktop.rs"):
            other=(ROOT / "melania-settings/src" / path).read_text()
            self.assertIn("melania_config::load()",other)

    def test_real_windows_operations(self):
        settings = (ROOT / "melania-settings/src/main.rs").read_text()
        files = (ROOT / "melania-files/src/main.rs").read_text()
        controls = (ROOT / "branding/window_controls.rs").read_text()
        chrome = (ROOT / "branding/window-chrome-win7.css").read_text()
        for code, header in ((settings, "&titlebar"), (files, "&aero_header")):
            self.assertIn('window_controls::install(&window, '+header+');', code)
            self.assertIn('include_str!("../../branding/window-chrome-win7.css")', code)
        for call in ("w.minimize()", "w.maximize()", "w.unmaximize()", "w.close()"):
            self.assertIn(call, controls)
        for cssclass in ("win7-minimize", "win7-maximize", "win7-close"):
            self.assertIn(cssclass, controls)
            self.assertIn(cssclass, chrome)
        # Both apps use the same CSS selector instead of app-specific controls.
        self.assertIn("headerbar.melania-win7-header", chrome)
        self.assertNotIn("headerbar.files-aero-header windowcontrols", chrome)

    def test_office_category_uses_original_suite_icon(self):
        sys.path.insert(0, str(ROOT / "tools"))
        from melania_office_artwork import office_icon
        from melania_fluent_reference import artwork
        for pixels in (16, 48, 128):
            svg=office_icon(pixels)
            ET.fromstring(svg)
            self.assertIn("url(#green)", svg if pixels > 24 else office_icon(128))
        self.assertEqual(artwork("applications-office", "apps", "light", 96),
                         office_icon(96))
        self.assertIsNone(artwork("libreoffice-writer", "apps", "light", 96))

if __name__=="__main__":
    unittest.main()
