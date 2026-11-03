"""Melania Files Nautilus tab restoration and real multiwindow lifecycle.

Source-level invariants complement Rust tests and nested GUI CI; neither
pretends to test drag-and-drop between separate physical Wayland windows.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
FILES=(ROOT/"melania-files/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()


class FilesTabRestoreDetach(unittest.TestCase):
    def test_closed_tabs_store_only_location_and_bound_history(self):
        for token in (
            "enum ClosedFilesTab {",
            "Local(PathBuf)",
            "Remote(String)",
            "const MAX_RECENT_FILES_TABS: usize = 16",
            "fn push_closed_file_tab(",
            "fn remember_closed_files_tab(",
            "if remember { remember_closed_files_tab(recently_closed,&title); }",
            "fn reopen_closed_files_tab(",
            "history.borrow_mut().pop()",
        ):
            self.assertIn(token,FILES)
        self.assertNotIn("Vec<(GtkBox,ClosedFilesTab)>",FILES)

    def test_keyboard_and_context_menu_restore_without_widget_reuse(self):
        for token in (
            '("Riapri ultima scheda chiusa",FilesTabAction::RestoreClosed)',
            "if shift {",
            "reopen_closed_files_tab(&recent_for_keys,&next_for_keys,&keys_add)",
            "next_for_add.borrow_mut().take().map(|closed|closed.location())",
            "new_button.emit_clicked()",
        ):
            self.assertIn(token,FILES)

    def test_detached_tab_opens_a_genuine_gtk_window(self):
        for token in (
            '("Sposta scheda in nuova finestra",FilesTabAction::MoveToWindow)',
            "create_files_window(&app,Some(place.location()));",
            "close_files_tab(&pages,&strip,&headers,&window,&page,&closed_for_actions,false);",
            "window.close();",
            "fn create_files_window(app: &Application, explicitly_requested: Option<String>)",
            "ApplicationWindow::builder().application(app)",
            "create_files_window(app,None)",
        ):
            self.assertIn(token,FILES)

    def test_normal_new_window_uses_active_uri(self):
        self.assertIn("if (key==gdk::Key::n || key==gdk::Key::N) && !shift",FILES)
        self.assertIn("create_files_window(&app,location);",FILES)
        self.assertIn("let detached_window=explicitly_requested.is_some();",FILES)
        self.assertIn("if !detached_window {",FILES)

    def test_last_tab_is_kept_for_normal_close(self):
        closer=FILES.split("fn close_files_tab(",1)[1].split("fn reorder_files_tab(",1)[0]
        self.assertIn("if notebook.n_pages()<=1{return;}",closer)
        self.assertIn("notebook.remove_page(Some(index));",closer)

    def test_rust_tests_include_lifo_and_location_validation(self):
        for name in (
            "remembers_local_and_remote_locations_without_any_gtk_resources",
            "only_the_last_sixteen_closed_tabs_are_kept_in_lifo_order",
        ):
            self.assertIn("fn "+name+"()",FILES)


if __name__=="__main__":
    unittest.main()
