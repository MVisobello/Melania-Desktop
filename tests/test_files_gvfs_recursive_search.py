"""Guard remote GIO recursive name search against unbounded or unsafe traversal.

Supplementary tests; the Rust tests exercise actual file:/// directory trees.
"""
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[1]
BACK=(ROOT/'melania-files/src/gio_backend.rs').read_text()
VIEW=(ROOT/'melania-files/src/gio_remote.rs').read_text()

class RemoteSearchGuards(unittest.TestCase):
    def test_bounded_no_symlink_follow_and_native_cancel(self):
        for value in ('MAX_REMOTE_SEARCH_DEPTH:u8=8',
                      'MAX_REMOTE_SEARCH_ITEMS:usize=5_000',
                      'MAX_REMOTE_SEARCH_FOLDERS:usize=300',
                      'MAX_REMOTE_SEARCH_RESULTS:usize=300',
                      'gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS',
                      'with_gio_cancel(cancelled,|provider|',
                      'next_file(Some(provider))'):
            self.assertIn(value,BACK)
        self.assertIn('if depth<MAX_REMOTE_SEARCH_DEPTH{',BACK)
        self.assertIn('report.skipped_folders+=1;',BACK)

    def test_explicit_search_off_ui_thread(self):
        self.assertIn('deep_search.connect_clicked(move |_|open_deep_search(&ui))',VIEW)
        self.assertIn('std::thread::spawn(move ||{\n        let result=gio_backend::search_tree(',VIEW)
        self.assertIn('stop_click.store(true,Ordering::Release);',VIEW)
        self.assertIn('search_progress.as_ref()',VIEW)
        self.assertIn('navigation(&selected,hit.uri.clone(),true);',VIEW)


if __name__=='__main__':
    unittest.main()
