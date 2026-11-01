"""Regression guards for identity-bound GVfs rename undo/redo.

These checks supplement the real Rust tests and desktop GUI CI. They do not
claim atomic identity verification on arbitrary third-party GVfs servers.
"""
from pathlib import Path
import unittest

BASE=Path(__file__).resolve().parents[1]
B=(BASE/"melania-files/src/gio_backend.rs").read_text()
U=(BASE/"melania-files/src/gio_remote.rs").read_text()

class RenameUndoGuards(unittest.TestCase):
    def test_identity_bound_receipt_and_noreplace(self):
        self.assertIn('pub struct RenameReceipt {',B)
        self.assertIn('info.attribute_string("id::file")',B)
        self.assertIn('info.attribute_string("id::filesystem")',B)
        self.assertIn('gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS',B)
        self.assertIn('pub fn rename_with_receipt(',B)
        self.assertIn('pub fn replay_rename(',B)
        self.assertIn('if identity.0!=receipt.file_id || identity.1!=receipt.filesystem_id{',B)
        self.assertIn('if dest.query_exists(',B)
        self.assertIn('rename_item(source,&name)?',B)
        self.assertIn('melania_transfer::rename_local(',B)
        self.assertNotIn('gio::FileCopyFlags::OVERWRITE',B)

    def test_undo_redo_receipts_only_after_success(self):
        for value in ('HistoryChange::New(receipt)',
                      'HistoryChange::Undo',
                      'HistoryChange::Redo',
                      'history.redo=history.undo.take()',
                      'history.undo=history.redo.take()',
                      'sync_history_buttons(&state)',
                      'run_job_history(&ui,'):
            self.assertIn(value,U)
        self.assertIn('let receipt=ui.rename_history.borrow().undo.clone()',U)
        self.assertIn('let receipt=ui.rename_history.borrow().redo.clone()',U)
        self.assertIn('rename_entry.has_focus()',U)
        self.assertIn('key==gdk::Key::z',U)
        self.assertIn('modifiers.contains(gdk::ModifierType::SHIFT_MASK)',U)

    def test_collision_and_stale_tests_present(self):
        for name in ('local_recorded_rename_can_undo_and_redo_without_overwrite',
                     'undo_rename_never_overwrites_a_new_destination',
                     'undo_refuses_changed_identity_at_renamed_path'):
            self.assertIn(name,B)

if __name__=="__main__":
    unittest.main()
