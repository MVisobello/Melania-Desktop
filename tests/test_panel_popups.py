#!/usr/bin/env python3
"""Guard the GTK layer-role workaround used for live notifications/calendar."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
PANEL = (ROOT / "melania-panel/src/main.rs").read_text()
PREFERENCES = (ROOT / "melania-panel/src/panel_preferences.rs").read_text()
SMOKE = (ROOT / "scripts/smoke-portable-bundle.sh").read_text()


class PanelLayerRoleRegression(unittest.TestCase):
    def test_notification_and_quick_do_not_unmap_layer_role(self):
        self.assertIn("fn hide_popup_surface(w:&ApplicationWindow)", PANEL)
        self.assertIn('w.add_css_class("melania-popup-concealed")', PANEL)
        self.assertIn("w.set_margin(Edge::Right, POPUP_HIDE_RIGHT_MARGIN)", PANEL)
        self.assertIn("w.set_keyboard_mode(KeyboardMode::None)", PANEL)
        self.assertIn("fn present_popup_surface(w:&ApplicationWindow)", PANEL)
        self.assertIn("w.set_keyboard_mode(KeyboardMode::OnDemand)", PANEL)
        for forbidden in (
            "qs.set_visible(false)",
            "nt.set_visible(false)",
            "notifications.set_visible(false)",
            "myself.set_visible(false);other.present()",
            "notify.set_visible(false)",
            "cal.set_visible(false)",
            "other.set_visible(false)",
        ):
            self.assertNotIn(forbidden, PANEL)

    def test_live_panel_auto_hide_ignores_concealed_popups(self):
        self.assertIn('!w.has_css_class("melania-popup-concealed")', PREFERENCES)
        self.assertIn("popup_is_open(&notify)", PANEL)
        self.assertIn("popup_is_open(&cal)", PANEL)

    def test_real_notification_signal_regression_is_in_bundle_smoke(self):
        self.assertIn("MELANIA_SMOKE_NOTIFICATION_SWITCH=1", SMOKE)
        self.assertIn("MELANIA_SMOKE_NOTIFICATION_CLICK_OK", PANEL)
        self.assertIn('click.emit_by_name::<()>("clicked",&[])', PANEL)
        self.assertIn("PASS: quick controls to notifications click preserves the panel client", SMOKE)
        self.assertIn("MELANIA_CLIENT_EXIT program=.*melania-panel", SMOKE)


if __name__ == "__main__":
    unittest.main(verbosity=2)
