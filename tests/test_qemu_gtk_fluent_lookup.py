"""The real QEMU GTK4 panel must resolve shipped Fluent icons, not just log a theme name."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class QemuGtkFluentLookup(unittest.TestCase):
    def test_real_panel_uses_live_gtk_icon_resolver(self):
        source = (ROOT / "melania-panel/src/main.rs").read_text()
        # GTK4 owns the display IconTheme singleton; selecting its theme
        # via set_theme_name() raises a critical. Settings updates the managed
        # resolver and the runtime has_icon() checks remain mandatory.
        self.assertIn('settings.set_property("gtk-icon-theme-name",selected_theme)', source)
        self.assertIn('let icons=gtk::IconTheme::for_display(&display)', source)
        self.assertNotIn('icons.set_theme_name(Some(selected_theme))', source)
        self.assertIn('MELANIA_QEMU_VERIFY_FLUENT_LOOKUP', source)
        self.assertIn('MELANIA_QEMU_GTK_FLUENT_ICON_LOOKUPS_PASS=true', source)
        self.assertIn('std::process::exit(53)', source)
        self.assertIn('icons.has_icon(name)', source)
        for icon in ("melania", "org.melaniadesktop.Files",
                     "org.melaniadesktop.Settings",
                     "melania-tray-wifi",
                     "network-wireless-signal-excellent-symbolic"):
            self.assertIn(f'"{icon}"', source)

    def test_three_guest_gtk_launches_are_actually_checked(self):
        guest = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
        self.assertEqual(guest.count("MELANIA_QEMU_VERIFY_FLUENT_LOOKUP=YES_DISPOSABLE_QEMU"), 3)
        self.assertEqual(guest.count("MELANIA_QEMU_GTK_FLUENT_ICON_LOOKUPS_PASS=true"), 3)
        self.assertIn("real_gtk_smoke_Fluent_icons_not_resolved_by_GTK4", guest)
        self.assertIn("real_gtk_KMS_Fluent_icons_not_resolved_by_GTK4", guest)
        self.assertIn("MELANIA_QEMU_REAL_GTK_PANEL_KMS_SCANOUT_PASS=true", guest)
        self.assertIn("MELANIA_QEMU_REAL_GTK_TWO_FRAME_KMS_SCANOUT_PASS=true", guest)


if __name__ == "__main__":
    unittest.main()
