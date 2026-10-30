"""Source-level guards for v0.38: not a substitute for Cargo/GTK testing."""
from pathlib import Path
import unittest
R=Path(__file__).resolve().parents[1]
GUI=(R/'melania-files/src/main.rs').read_text()
WORKER=(R/'melania-transfer/src/background.rs').read_text()

class Files038(unittest.TestCase):
    def test_new_background_module_is_exported(self):
        self.assertIn('pub mod background;', (R/'melania-transfer/src/lib.rs').read_text())
        self.assertIn('fn start_background_copy',GUI)
        self.assertIn('pub fn start_copy(',WORKER)

    def test_background_is_cancellable(self):
        for token in ('AtomicBool','Event::Cancelled','Event::Progress','Event::Finished',
                      'remove_new_root(&target)', 'create_new(true)', 'check(stop)?'):
            self.assertIn(token,WORKER)
        self.assertIn('job.store(true, Ordering::Release)', GUI)
        self.assertIn('glib::timeout_add_local',GUI)

    def test_multiselection_and_keyboard(self):
        for token in ('selected_paths: Rc<RefCell<BTreeSet<PathBuf>>>','ModifierType::CONTROL_MASK',
                      'ModifierType::SHIFT_MASK','selected.extend(visible_for_all.borrow()',
                      'paths.iter().map(gio::File::for_path)', 'browser.selected_paths.borrow_mut().retain'):
            self.assertIn(token,GUI)

    def test_move_is_atomic_and_avoids_overwrite(self):
        original=(R/'melania-transfer/src/lib.rs').read_text()
        self.assertIn('RENAME_NOREPLACE',original)
        self.assertIn('transfer::move_local(&path,&target)',GUI)
        self.assertIn('Some(PendingTransfer{paths:remaining,is_move:true})',GUI)

    def test_rust_unit_tests_cover_critical_cases(self):
        for test in ('copies_files_and_reports_complete_progress','fails_without_overwriting',
                     'cancellation_preserves_source', 'does_not_follow_symlink'):
            self.assertIn(test,WORKER)

if __name__=='__main__': unittest.main()
