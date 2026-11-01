#!/usr/bin/env python3
"""Working panel preferences are persisted, live and confined to Melania."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
CONFIG=(ROOT/"melania-config/src/lib.rs").read_text()
PANEL=(ROOT/"melania-panel/src/main.rs").read_text()
LIVE=(ROOT/"melania-panel/src/panel_preferences.rs").read_text()
SETTINGS=(ROOT/"melania-settings/src/xfce_panel.rs").read_text()
PAGES=(ROOT/"melania-settings/src/xfce_pages.rs").read_text()

class PanelFunctionalSettings(unittest.TestCase):
    def test_config_validates_defaults_and_old_profiles(self):
        for key in ("panel_height","panel_position","panel_auto_hide","panel_opacity"):
            self.assertIn(f"pub {key}:",CONFIG)
            self.assertIn(f'"{key}" =>',CONFIG) if key!="panel_position" else self.assertIn('"panel_position" if',CONFIG)
        self.assertIn('panel_height: 52, panel_position: "bottom".into()',CONFIG)
        self.assertIn('panel_auto_hide: false, panel_opacity: 94',CONFIG)
        self.assertIn('fn panel_controls_validate_and_keep_approved_defaults()',CONFIG)

    def test_settings_are_operational_and_offer_reset(self):
        self.assertIn('"panel" => crate::xfce_panel::page()',PAGES)
        for feature in ('"Altezza del pannello"','"Posizione dello schermo"',
                        '"Nascondi automaticamente"','"Opacità del pannello"'):
            self.assertIn(feature,SETTINGS)
        self.assertIn('save.connect_clicked(move |_|',SETTINGS)
        self.assertIn('reset.connect_clicked(move |_|',SETTINGS)
        self.assertIn('melania_config::load()',SETTINGS)
        self.assertIn('melania_config::save(&latest)',SETTINGS)

    def test_real_layer_shell_surface_and_popups_are_reanchored(self):
        self.assertIn('mod panel_preferences;',PANEL)
        self.assertIn('panel_preferences::attach_live_preferences(',PANEL)
        for phrase in ('panel.set_anchor(Edge::Top,top)', 'panel.set_anchor(Edge::Bottom,!top)',
                       'window.set_exclusive_zone(', 'relocate(start,top',
                       'relocate(quick,top','relocate(calendar,top',
                       'relocate(notifications,top','provider.load_from_data('):
            self.assertIn(phrase,LIVE)
        self.assertIn('Duration::from_millis(500)',LIVE)
        self.assertIn('Duration::from_millis(650)',LIVE)
        self.assertIn('melania-start-concealed',LIVE)

    def test_existing_start_clock_remain_unmodified(self):
        self.assertIn('clock_row.append(&time_label);',PANEL)
        self.assertIn('clock_row.append(&date_label);',PANEL)
        self.assertIn('clock_row.set_size_request(84,-1);',PANEL)
        self.assertIn('menu.set_size_request(43,43);',PANEL)
        self.assertIn('bar.append(&clock_separator);',PANEL)

if __name__=="__main__":
    unittest.main()
