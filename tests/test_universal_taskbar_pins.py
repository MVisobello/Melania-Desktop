#!/usr/bin/env python3
"""Taskbar pinning contracts: real freedesktop app IDs and safe persistence."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text(encoding="utf-8")
CSS=(ROOT/"melania-panel/assets/panel.css").read_text(encoding="utf-8")

class UniversalPinning(unittest.TestCase):
    def test_no_two_launcher_whitelist(self):
        self.assertIn('fn installed_pin_app(id:&str)->Option<gio::AppInfo>',PANEL)
        self.assertIn('app.id().as_deref()==Some(id)',PANEL)
        self.assertIn('app.should_show()',PANEL)
        self.assertIn('fn app_button(id:&str)->Option<Button>',PANEL)
        self.assertNotIn('if !["melania-files","melania-settings"].contains(&src.as_str())',PANEL)
        self.assertIn('let launch_id=id.to_string();',PANEL)
        self.assertIn('installed_pin_app(&launch_id)',PANEL)
        self.assertIn('app.launch(&[],None::<&gio::AppLaunchContext>)',PANEL)

    def test_user_pins_are_persisted_securely_and_removable(self):
        self.assertIn('fn save_pins(ids:&[String])->std::io::Result<()>',PANEL)
        self.assertIn('create_new(true).mode(0o600)',PANEL)
        self.assertIn('file.sync_all()?;',PANEL)
        self.assertIn('fs::rename(&temp,&path)',PANEL)
        self.assertIn('fn toggle_pin(id:&str)',PANEL)
        self.assertIn('pins.remove(n);',PANEL)
        self.assertNotIn('for id in DEFAULT {if !ids.iter()',PANEL)
        self.assertIn('if !path.exists(){return DEFAULT',PANEL)

    def test_start_context_and_taskbar_live_refresh(self):
        self.assertIn('attach_pin_context(&button,program.to_string());',PANEL)
        self.assertIn('attach_pin_context(&button,id);',PANEL)
        self.assertIn('click.set_button(3);',PANEL)
        self.assertIn('"Rimuovi dalla barra"',PANEL)
        self.assertIn('"Aggiungi alla barra"',PANEL)
        self.assertIn('populate_pins(&pinned,&pinned_buttons,&pinned_order);',PANEL)
        self.assertIn('if load_pins()!=*pinned_ids.borrow(){',PANEL)
        self.assertIn('Duration::from_millis(800)',PANEL)
        self.assertIn('if last.borrow().as_ref() == Some(&windows) {return;}',PANEL)
        self.assertIn('w.app_id.trim_end_matches(".desktop").eq_ignore_ascii_case(app_id)',PANEL)
        self.assertIn('melania-pin-popover',CSS)

if __name__=="__main__":
    unittest.main(verbosity=2)
