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
        self.assertIn("trailing.append(&workspace_switcher());",PANEL)
        self.assertNotIn("bar.append(&workspace_strip());",PANEL)
        self.assertNotIn("fn workspace_strip()",PANEL)
        self.assertLess(PANEL.index("bar.append(&running_scroll);"),
                        PANEL.index("trailing.append(&workspace_switcher());"))
        self.assertLess(PANEL.index("let space=GtkBox::new",PANEL.index("bar.append(&running_scroll);")),
                        PANEL.index("trailing.append(&workspace_switcher());"))
        self.assertLess(PANEL.index("trailing.append(&workspace_switcher());"),
                        PANEL.index("trailing.append(&status);"))

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
        self.assertIn('trailing.append(&clock_separator);',PANEL)
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
        self.assertLess(taskbar.index('trailing.append(&notification_button);'),taskbar.index('trailing.append(&clock);'))
        self.assertLess(taskbar.index('trailing.append(&clock);'),taskbar.index('w.set_child(Some(&bar));w'))
        self.assertIn(".melania-bar .melania-clock-date",CSS)
        self.assertIn(".melania-bar .melania-clock-time",CSS)

    def test_tray_is_compact_and_clock_lines_have_equal_size(self):
        self.assertIn('trailing.add_css_class("melania-trailing");',PANEL)
        self.assertIn('bar.append(&trailing);',PANEL)
        self.assertIn('let status_row=GtkBox::new(Orientation::Horizontal,3);',PANEL)
        self.assertIn('.melania-bar .melania-trailing {',CSS)
        self.assertIn('border-spacing: 2px;',CSS)
        self.assertIn('.melania-bar .melania-trailing .melania-status-icons {',CSS)
        self.assertIn('.melania-bar .melania-trailing .melania-clock-time,',CSS)
        self.assertIn('.melania-bar .melania-trailing .melania-clock-date {',CSS)
        self.assertIn('font-size: 12px;',CSS)
        self.assertIn('font-weight: 400;',CSS)

    def test_clock_is_vertically_centered_with_slightly_larger_text(self):
        taskbar=PANEL[PANEL.index('fn taskbar('):PANEL.index('fn install_start_hotkey_listener(')]
        self.assertIn('trailing.set_valign(gtk::Align::Center);',taskbar)
        self.assertIn('clock.set_valign(gtk::Align::Center);',taskbar)
        self.assertIn('clock_row.set_valign(gtk::Align::Center);',taskbar)
        self.assertIn('clock_row.set_vexpand(false);',taskbar)
        self.assertIn('/* Keep both clock lines legible',CSS)
        clock_rule=CSS.split('/* Keep both clock lines legible',1)[1]
        self.assertIn('font-size: 13px;',clock_rule)
        self.assertIn('font-weight: 400;',clock_rule)

    def test_clock_has_no_expandable_blank_area_between_tray_and_time(self):
        taskbar=PANEL[PANEL.index('fn taskbar('):PANEL.index('fn install_start_hotkey_listener(')]
        self.assertIn('trailing.set_hexpand(false);',taskbar)
        self.assertIn('trailing.set_halign(gtk::Align::End);',taskbar)
        self.assertIn('clock.set_hexpand(false);',taskbar)
        self.assertIn('clock.set_halign(gtk::Align::Start);',taskbar)
        self.assertIn('let trailing=GtkBox::new(Orientation::Horizontal,2);',taskbar)
        self.assertLess(taskbar.index('trailing.append(&status);'),
                        taskbar.index('trailing.append(&notification_button);'))
        self.assertLess(taskbar.index('trailing.append(&notification_button);'),
                        taskbar.index('trailing.append(&clock_separator);'))
        self.assertLess(taskbar.index('trailing.append(&clock_separator);'),
                        taskbar.index('trailing.append(&clock);'))
        self.assertIn('min-width: 84px;',CSS)
        self.assertIn('margin: 8px 2px 8px 1px;',CSS)

if __name__=="__main__":
    unittest.main(verbosity=2)
