#!/usr/bin/env python3
"""Regression checks for the real GTK taskbar workspace popup and right clock."""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text(encoding="utf-8")
CSS=(ROOT/"melania-panel/assets/panel.css").read_text(encoding="utf-8")

class TaskbarDesktopClock(unittest.TestCase):
    def test_one_workspace_icon_near_tray_not_center_strip(self):
        self.assertIn("fn workspace_switcher() -> gtk::MenuButton",PANEL)
        self.assertIn("bar.append(&workspace_switcher());",PANEL)
        self.assertNotIn("bar.append(&workspace_strip());",PANEL)
        self.assertNotIn("fn workspace_strip()",PANEL)
        self.assertLess(PANEL.index("bar.append(&running_scroll);"),
                        PANEL.index("bar.append(&workspace_switcher());"))
        self.assertLess(PANEL.index("let space=GtkBox::new",PANEL.index("bar.append(&running_scroll);")),
                        PANEL.index("bar.append(&workspace_switcher());"))
        self.assertLess(PANEL.index("bar.append(&workspace_switcher());"),
                        PANEL.index("bar.append(&status);"))

    def test_all_desktops_reachable_and_real_ipc_is_used(self):
        self.assertIn("for n in 1..=count {",PANEL)
        self.assertIn('Button::with_label(&format!("Desktop {n}"))',PANEL)
        self.assertIn("WindowCommand::Workspace(n)",PANEL)
        self.assertIn("current_workspace(path)",PANEL)
        self.assertIn("choice.add_css_class(\"melania-workspace-active\");",PANEL)
        self.assertIn("choice.remove_css_class(\"melania-workspace-active\");",PANEL)
        self.assertIn("toggle.set_popover(Some(&popover));",PANEL)
        self.assertIn("popover.set_position(gtk::PositionType::Top);",PANEL)
        self.assertIn("popup.popdown();",PANEL)
        self.assertIn('format!("Desktop {active} · Cambia desktop")',PANEL)
        self.assertIn("Duration::from_millis(900)",PANEL)

    def test_custom_vector_not_dependent_on_icon_pack(self):
        self.assertIn('symbol.set_draw_func(move |_,cr,width,height|',PANEL)
        self.assertIn('symbol.set_content_width(21);',PANEL)
        self.assertIn('rounded(1.2,6.0,10.0,8.5,1.6);',PANEL)
        self.assertIn('rounded(12.8,6.0,10.0,8.5,1.6);',PANEL)
        self.assertIn('refresh_label.set_label(&active.to_string());',PANEL)
        self.assertIn(".melania-workspace-switcher-content",CSS)
        self.assertIn(".melania-workspace-menu button.melania-workspace-choice.melania-workspace-active",CSS)
        self.assertIn("window.melania-theme-light .melania-workspace-menu",CSS)

    def test_time_above_date_at_right_and_single_separator(self):
        self.assertIn('GtkBox::new(Orientation::Vertical,0);',PANEL)
        self.assertIn('clock_row.set_halign(gtk::Align::End);',PANEL)
        self.assertIn('clock_separator=gtk::Separator::new(Orientation::Vertical);',PANEL)
        self.assertIn('bar.append(&clock_separator);',PANEL)
        self.assertIn('badge.set_visible(false);',PANEL)
        self.assertIn('badge.set_visible(unread>0);',PANEL)
        self.assertIn('separator.melania-clock-separator',CSS)
        self.assertIn('min-width: 1px;',CSS)

        self.assertIn('clock_row.append(&date_label);',PANEL)
        self.assertIn('clock_row.append(&time_label);',PANEL)
        self.assertIn('date_label.set_xalign(1.0);',PANEL)
        self.assertIn('time_label.set_xalign(1.0);',PANEL)
        self.assertIn('clock_row.set_size_request(84,-1);',PANEL)
        self.assertIn('date_label.set_halign(gtk::Align::Fill);',PANEL)
        self.assertIn('time_label.set_halign(gtk::Align::Fill);',PANEL)
        self.assertIn('date_label.set_hexpand(true);',PANEL)
        self.assertIn('time_label.set_hexpand(true);',PANEL)
        self.assertIn('t.format("%d/%m/%Y")',PANEL)
        self.assertIn('t.format("%H:%M")',PANEL)
        self.assertLess(PANEL.index('clock_row.append(&time_label);'),PANEL.index('clock_row.append(&date_label);'))
        clock_start=PANEL.index('fn taskbar(')
        clock_end=PANEL.index('fn install_start_hotkey_listener(',clock_start)
        taskbar=PANEL[clock_start:clock_end]
        self.assertLess(taskbar.index('bar.append(&notification_button);'),taskbar.index('bar.append(&clock);'))
        self.assertLess(taskbar.index('bar.append(&clock);'),taskbar.index('w.set_child(Some(&bar));w'))
        self.assertIn(".melania-bar .melania-clock-date",CSS)
        self.assertIn(".melania-bar .melania-clock-time",CSS)

if __name__=="__main__":
    unittest.main(verbosity=2)
