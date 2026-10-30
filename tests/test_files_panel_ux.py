#!/usr/bin/env python3
"""UI contract tests for Files empty-state feedback and panel Esc dismissal."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]

class FilesAndPanelUX(unittest.TestCase):
    def test_files_has_meaningful_empty_states(self):
        source=(ROOT/"melania-files/src/main.rs").read_text()
        stylesheet=(ROOT/"melania-files/assets/style.css").read_text()
        self.assertIn("fn empty_folder_message(",source)
        self.assertIn('("system-search", "Nessun risultato"',source)
        self.assertIn('("image-x-generic", "Nessuna immagine visibile"',source)
        self.assertIn("files_overlay.add_overlay(&empty_view)",source)
        self.assertIn("browser.empty_view.set_visible(is_empty)",source)
        self.assertIn("browser.empty_icon.set_icon_name(Some(icon))",source)
        self.assertIn("files-empty-state",stylesheet)

    def test_files_sidebar_toggle_uses_real_window_widgets(self):
        source=(ROOT/"melania-files/src/main.rs").read_text()
        self.assertIn('view-sidebar-symbolic',source)
        self.assertIn('sidebar_to_toggle.set_visible(!sidebar_to_toggle.is_visible())',source)
        self.assertIn('key == gdk::Key::b',source)
        self.assertIn('do_sidebar.emit_clicked()',source)

    def test_panel_escape_dismisses_only_popup_surface(self):
        source=(ROOT/"melania-panel/src/main.rs").read_text()
        self.assertIn('if exclusive == 0 {',source)
        self.assertIn('gdk::Key::Escape',source)
        self.assertIn('surface.set_visible(false)',source)
        self.assertIn('glib::Propagation::Stop',source)

if __name__=="__main__":
    unittest.main(verbosity=2)
