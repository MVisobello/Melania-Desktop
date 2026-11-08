"""Aero theme regression: real GTK alpha, live settings and separate fallback.

This tests source integration/semantics. Actual pixel blending still requires
the nested Wayland screenshot job; native GPU blur remains a separate gate.
"""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PANEL = (ROOT / "melania-panel/src/panel_preferences.rs").read_text()
CSS = {
    "panel": (ROOT / "melania-panel/assets/panel.css").read_text(),
    "settings": (ROOT / "melania-settings/assets/style.css").read_text(),
    "files": (ROOT / "melania-files/assets/style.css").read_text(),
    "chrome": (ROOT / "branding/window-chrome-win7.css").read_text(),
}
SETTINGS = (ROOT / "melania-settings/src/main.rs").read_text()
FILES = (ROOT / "melania-files/src/main.rs").read_text()


class AeroGlassLive(unittest.TestCase):
    def test_panel_glass_alpha_tracks_preferences_and_real_layer_surfaces(self):
        for token in (
            'fn glass_opacity(prefs:&Preferences)->f64',
            'prefs.appearance_theme()=="aero"',
            "prefs.high_contrast",
            "prefs.blur_strength",
            "prefs.panel_opacity",
            "provider.load_from_data(&format!",
            "window.melania-theme-aero:not(.melania-high-contrast) .melania-bar",
            "for surface in [panel,start,quick,notifications,calendar]",
            'surface.add_css_class(&format!("melania-theme-{}"',
        ):
            self.assertIn(token, PANEL)

    def test_real_glass_is_not_applied_to_opaque_alternatives(self):
        for token in ("window.melania-theme-aero:not(.melania-high-contrast)",
                      "window.melania-theme-light", "window.melania-theme-dark"):
            self.assertIn(token, PANEL)
        self.assertIn("window.melania-theme-aero.melania-high-contrast", CSS["panel"])
        self.assertIn("melania-theme-aero.high-contrast", CSS["chrome"])
        self.assertIn(".settings-root.high-contrast", CSS["settings"])

    def test_all_three_glass_surfaces_have_wayland_alpha(self):
        for kind, selector in (
            ("panel", "window.melania-theme-aero:not(.melania-high-contrast) .melania-start-panel"),
            ("settings", "window.melania-settings-window.melania-theme-aero:not(.high-contrast)"),
            ("files", "window.melania-files-window.melania-theme-aero:not(.high-contrast)"),
        ):
            self.assertIn(selector, CSS[kind])
            self.assertIn("rgba(", CSS[kind])
        self.assertIn("headerbar.melania-win7-header.melania-theme-aero", CSS["chrome"])

    def test_settings_window_classes_change_without_restart(self):
        self.assertIn("fn refresh_theme(window: &ApplicationWindow", SETTINGS)
        self.assertIn("window.add_css_class(&theme_class)", SETTINGS)
        self.assertIn('window.remove_css_class(name)', SETTINGS)
        self.assertIn('window.add_css_class("high-contrast")', SETTINGS)
        self.assertIn('if appearance.high_contrast', FILES)

    def test_css_does_not_claim_or_use_unavailable_backdrop_filter(self):
        # GTK CSS translucent backgrounds are NOT the GPU blur shader pass.
        for name, css in CSS.items():
            self.assertNotIn("backdrop-filter:", css, name)
        self.assertIn("sfocatura GPU nativa in sviluppo", SETTINGS)


if __name__ == "__main__":
    unittest.main()
