#!/usr/bin/env python3
"""Desktop background preferences apply to real Melania layer, not host DE."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
CONFIG=(ROOT/"melania-config/src/lib.rs").read_text()
SURFACE=(ROOT/"melania-desktop-surface/src/main.rs").read_text()
SETTINGS=(ROOT/"melania-settings/src/xfce_desktop.rs").read_text()
PAGES=(ROOT/"melania-settings/src/xfce_pages.rs").read_text()

class DesktopSettings(unittest.TestCase):
    def test_config_is_validated_and_preserves_original(self):
        for name in ("desktop_wallpaper","desktop_wallpaper_fit","desktop_show_icons"):
            self.assertIn(f"pub {name}:",CONFIG)
        self.assertIn('desktop_wallpaper: String::new()',CONFIG)
        self.assertIn('desktop_wallpaper_fit: "cover".into()',CONFIG)
        self.assertIn('desktop_show_icons: true',CONFIG)
        self.assertIn("wallpaper_preferences_validate_and_preserve_old_visuals",CONFIG)

    def test_real_live_desktop_surface(self):
        self.assertIn("fn wallpaper_path(prefs:&melania_config::Preferences)",SURFACE)
        self.assertIn("std::env::var_os(\"MELANIA_WALLPAPER\")",SURFACE)
        self.assertIn("wallpaper_fit(&initial_prefs.desktop_wallpaper_fit)",SURFACE)
        self.assertIn("layer.set_visible(initial_prefs.desktop_show_icons);",SURFACE)
        self.assertIn("live_picture.set_file(Some(&selected));",SURFACE)
        self.assertIn("live_icons.set_visible(next.desktop_show_icons);",SURFACE)
        self.assertIn("Duration::from_millis(700)",SURFACE)

    def test_settings_apply_and_restore_safely(self):
        self.assertIn('stack.add_titled(&crate::xfce_desktop::page(),',PAGES)
        self.assertIn('Some("xfce-desktop")',PAGES)
        self.assertIn('("Scrivania e sfondo", "xfce-desktop"', (ROOT/"melania-settings/src/main.rs").read_text())
        self.assertIn('gtk::FileChooserNative::new(',SETTINGS)
        self.assertIn('valid_wallpaper(&image)',SETTINGS)
        self.assertIn('40_000_000',SETTINGS)
        self.assertIn('apply.connect_clicked(move |_|',SETTINGS)
        self.assertIn('reset.connect_clicked(move |_|',SETTINGS)
        self.assertIn('melania_config::save(&latest)',SETTINGS)
        self.assertIn('latest.desktop_show_icons=visibility.is_active();',SETTINGS)
        self.assertIn('latest.desktop_wallpaper_fit=match mode.selected()',SETTINGS)
        self.assertNotIn('Command::new("gsettings")',SETTINGS)
        self.assertNotIn('Command::new("xfconf-query")',SETTINGS)
if __name__=="__main__":
    unittest.main()
