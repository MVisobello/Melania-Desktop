"""Melania Files GVfs worker backpressure / stale-result invariants.

Supplemental structural checks; CI also builds Rust/GTK and starts a genuine
Wayland desktop with a file:/// GVfs tab.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
UI=(ROOT/"melania-files/src/gio_remote.rs").read_text()
GIO=(ROOT/"melania-files/src/gio_backend.rs").read_text()

class AsyncGVfsListing(unittest.TestCase):
    def test_one_worker_per_tab_with_replaceable_pending_request(self):
        for token in ('listing_running: Rc<Cell<bool>>',
                      'listing_pending: Rc<RefCell<Option<RemoteListingRequest>>>',
                      'if remote.listing_running.get(){return;}',
                      'remote.listing_pending.borrow_mut().take()',
                      '*remote.listing_pending.borrow_mut()=Some(RemoteListingRequest{',
                      'start_listing_worker(remote);'):
            self.assertIn(token,UI)
        refresh=UI.split('fn refresh(remote:&Remote) {',1)[1].split('fn format_size(',1)[0]
        self.assertNotIn('std::thread::spawn(',refresh)
        worker=UI.split('fn start_listing_worker(remote:&Remote) {',1)[1].split('fn refresh(remote:&Remote)',1)[0]
        self.assertEqual(worker.count('std::thread::spawn('),1)

    def test_old_results_never_render_and_worker_resumes_latest(self):
        self.assertIn('if ui.generation.get()==gen {',UI)
        self.assertIn('ui.listing_running.set(false);',UI)
        self.assertIn('start_listing_worker(&ui);',UI)
        self.assertIn('old.store(true,Ordering::Release)',UI)
        self.assertIn('cleanup.listing_pending.borrow_mut().take()',UI)
        self.assertIn('if cancelled.load(Ordering::Acquire){',GIO)


if __name__=="__main__":
    unittest.main()
