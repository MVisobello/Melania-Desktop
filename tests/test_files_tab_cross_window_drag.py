"""GTK4 tab drag and drop lifecycle contracts.

Verifies that tab-to-window operations remain separate from COPY-only file
drag/drop. Rust tests cover cancel reasons; the nested Wayland workflow must
still validate actual gesture dispatch before merging this feature.
"""
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"melania-files/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()


class FilesTabDnD(unittest.TestCase):
    def test_process_local_ticket_prevents_forged_uri_or_file_drops(self):
        for marker in (
            "struct FilesTabDrag {",
            "static ACTIVE_FILES_TAB_DRAG: RefCell<Option<FilesTabDrag>>",
            "fn active_files_tab_drag(token: &str) -> Option<FilesTabDrag>",
            "drag.token==token && drag.source_is_alive()",
            "fn clear_files_tab_drag(token: &str)",
            "source_notebook: glib::WeakRef<gtk::Notebook>",
            "source_page: glib::WeakRef<GtkBox>",
        ):
            self.assertIn(marker,SOURCE)

    def test_tearoff_on_no_target_only_not_escape_or_error(self):
        for marker in (
            "fn detach_files_tab_on_cancel(reason: gdk::DragCancelReason) -> bool",
            "matches!(reason,gdk::DragCancelReason::NoTarget)",
            "fn detach_dragged_files_tab(token: &str,reason: gdk::DragCancelReason)",
            "if !detach_files_tab_on_cancel(reason)",
            "create_files_window(&app,Some(drag.location.location()))",
            "if app.windows().len()<=before{return false;}",
            "DragCancelReason::UserCancelled",
            "DragCancelReason::Error",
        ):
            self.assertIn(marker,SOURCE)

    def test_gtk_gestures_reorder_or_transfer_to_other_window(self):
        for marker in (
            "drag_source.connect_drag_begin(",
            "drag_source.connect_drag_cancel(",
            "drag_source.connect_drag_end(",
            "drop.connect_drop(move |_,value,x,_|",
            "rail_drop.connect_drop(move |_,value,_,_|",
            "tab_rail.add_controller(rail_drop);",
            "import_dragged_files_tab(&token,&window,&pages,&add,&target_next)",
            "let destination=target.saturating_add(u32::from(right_half));",
        ):
            self.assertIn(marker,SOURCE)

    def test_no_dangling_or_duplicate_source_after_successful_import(self):
        action=SOURCE.split("fn import_dragged_files_tab(",1)[1].split(
            "fn detach_dragged_files_tab(",1)[0]
        for marker in ("target_notebook.n_pages()>before", "clear_files_tab_drag(token)",
                       "(drag.finish_move)();", "target_next.borrow_mut().take()"):
            # Use the inverse comparison in source to detect failed creation.
            if marker=="target_notebook.n_pages()>before":
                self.assertIn("target_notebook.n_pages()<=before",action)
            else:
                self.assertIn(marker,action)

    def test_file_drag_remains_copy_only(self):
        file_drag=SOURCE.split("fn attach_drag(",1)[1].split("fn show_properties(",1)[0]
        self.assertIn("source.set_actions(gdk::DragAction::COPY)",file_drag)
        self.assertNotIn("source.set_actions(gdk::DragAction::MOVE)",file_drag)

    def test_rust_covers_user_cancel_and_unregistered_tokens(self):
        self.assertIn("fn escape_and_errors_do_not_tear_off_or_create_windows()",SOURCE)
        self.assertIn("fn arbitrary_unregistered_string_cannot_trigger_tab_transfer()",SOURCE)


if __name__ == "__main__":
    unittest.main()
