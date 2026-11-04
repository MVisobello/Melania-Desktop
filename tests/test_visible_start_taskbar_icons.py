"""Regress real Melania Files/Settings color fallbacks and centered taskbar icons.

The generated Fluent icon theme is optional in standalone Mint/labwc; PNG
sources must be compiled into the GTK panel for portable deployments.
"""
from pathlib import Path
import struct
import unittest

ROOT = Path(__file__).resolve().parents[1]
PANEL = (ROOT/"melania-panel/src/main.rs").read_text()
CSS = (ROOT/"melania-panel/assets/panel.css").read_text()

class VisibleCoreIcons(unittest.TestCase):
    def test_both_packaged_color_png_assets_are_real_nonempty(self):
        for name in ("Files", "Settings"):
            path = ROOT / f"branding/Melania/64x64/apps/org.melaniadesktop.{name}.png"
            data = path.read_bytes()
            self.assertGreater(len(data),300)
            self.assertEqual(data[:8],b"\x89PNG\r\n\x1a\n")
            w,h=struct.unpack_from(">II",data,16)
            self.assertEqual((w,h),(64,64))
            self.assertIn(f'../../branding/Melania/64x64/apps/org.melaniadesktop.{name}.png',PANEL)
            self.assertIn(f'"org.melaniadesktop.{name}"',PANEL)
        self.assertIn("gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes))",PANEL)
        self.assertIn("Image::from_paintable(Some(&texture))",PANEL)
        self.assertIn('image.add_css_class("melania-color-app-icon")',PANEL)

    def test_installed_fluent_pack_precedes_embedded_fallback(self):
        self.assertIn('name.starts_with("Melania-Fluent-")',PANEL)
        self.assertIn('(!active_fluent_theme() || !available)',PANEL)
        self.assertIn("packaged_core_app_icon(primary)",PANEL)
        self.assertIn("Image::from_icon_name(if available { primary } else { fallback })",PANEL)
        self.assertIn('icon_with_fallback("org.melaniadesktop.Files","system-file-manager")',PANEL)
        self.assertIn('icon_with_fallback("org.melaniadesktop.Settings","preferences-system")',PANEL)

    def test_pinned_buttons_have_no_offset_one_child_box(self):
        start=PANEL.index("fn app_button(id:&str)->Option<Button>")
        end=PANEL.index("fn populate_pins(",start)
        pinned=PANEL[start:end]
        self.assertNotIn("let row=GtkBox::new(Orientation::Horizontal,6);",pinned)
        self.assertNotIn("row.append(&icon);",pinned)
        self.assertIn("b.set_child(Some(&icon));",pinned)
        self.assertIn("icon.set_halign(gtk::Align::Center);",pinned)
        self.assertIn("icon.set_valign(gtk::Align::Center);",pinned)

    def test_running_and_group_icons_use_same_centering_policy(self):
        start=PANEL.index("fn running_window_strip(")
        end=PANEL.index("fn workspace_switcher()",start)
        running=PANEL[start:end]
        self.assertIn("entry.set_halign(gtk::Align::Center);",running)
        self.assertIn("entry.set_valign(gtk::Align::Center);",running)
        self.assertIn("focus.set_child(Some(&window_icon));",running)
        self.assertIn("window_icon.set_halign(gtk::Align::Center);",running)
        self.assertIn("window_icon.set_valign(gtk::Align::Center);",running)
        self.assertIn("taskbar_app_icon(&grouped_window.app_id)",running)
        self.assertIn('icon.add_css_class("melania-taskbar-app-icon")',running)
        self.assertIn('window_icon.add_css_class("melania-taskbar-app-icon")',running)
        self.assertIn(".melania-bar .melania-taskbar-app-icon",CSS)
        self.assertIn(".melania-bar .melania-color-app-icon",CSS)
        self.assertIn("opacity: 1;",CSS)

if __name__ == "__main__":
    unittest.main()
