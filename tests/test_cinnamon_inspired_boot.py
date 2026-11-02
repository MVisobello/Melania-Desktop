"""Cinnamon-inspired boot phases and on-demand Start app catalog tests."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text()
COMPOSITOR=(ROOT/"melania-compositor/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class PhasedSessionAndStartTests(unittest.TestCase):
    def test_start_installed_catalog_is_not_eagerly_created(self):
        start=PANEL[PANEL.index("fn start_window("):PANEL.index("fn quick_window(")]
        self.assertIn("let installed_once=Rc::new(Cell::new(false))",start)
        self.assertIn('if !surface.is_visible() || installed_once.replace(true){return;}',start)
        self.assertIn('Duration::from_millis(75)',start)
        self.assertIn('gio::AppInfo::all().into_iter().filter',start)
        self.assertIn('VecDeque::from(apps)',start)
        self.assertIn('for _ in 0..12 {',start)
        self.assertIn('Duration::from_millis(16)',start)
        self.assertIn('installed_box.append(&button);',start)
        self.assertIn('searchable.borrow_mut().push((button,name_lower,false))',start)
        self.assertLess(start.index('let installed_once='),start.index('scroll.set_child(Some(&apps_box))'))

    def test_existing_search_filter_and_actions_remain_live(self):
        self.assertIn('query.connect_changed(move |q|',PANEL)
        self.assertIn('searchable_for_filter.borrow().iter()',PANEL)
        self.assertIn('let first=searchable_for_enter.borrow().iter()',PANEL)
        self.assertIn('if let Some(button)=first {button.emit_clicked();}',PANEL)
        self.assertIn('start_history::record_launch(id);',PANEL)
        self.assertIn('attach_pin_context_trusted(&button,id);',PANEL)
        self.assertIn('hide_start_surface(&window);',PANEL)

    def test_cold_gtk_processes_start_sequentially_only_after_shell(self):
        self.assertIn('VecDeque<OsString>',COMPOSITOR)
        self.assertIn('next_application_start:Option<Instant>=None;',COMPOSITOR)
        self.assertIn('MELANIA_BOOT_APP_STARTED_MS=',COMPOSITOR)
        self.assertIn('applications.pop_front()',COMPOSITOR)
        self.assertIn('Duration::from_millis(180)',COMPOSITOR)
        self.assertIn('if stage_shell {',COMPOSITOR)
        self.assertIn('fallback=!shell_frame_painted',COMPOSITOR)
        self.assertIn('started[1]-started[0]>=150',SMOKE)

if __name__ == "__main__":
    unittest.main()
