#!/usr/bin/env python3
"""Files interaction responsiveness and stale filesystem monitor regressions.

These contract checks complement (not replace) Rust tests and real GTK smoke.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
LOCAL = (ROOT / "melania-files/src/main.rs").read_text()
REMOTE = (ROOT / "melania-files/src/gio_remote.rs").read_text()


class FilesNonblockingInteractions(unittest.TestCase):
    def test_local_cut_and_trash_delegate_to_worker(self):
        cut = LOCAL.split("let pending=state.pending.borrow().clone();",1)[1].split(
            "if let Some(pending)=pending.filter(|p|p.is_move)",1)[1].split(
            "return;",1)[0]
        trash = LOCAL.split("trash_button.connect_clicked(move |_| {",1)[1].split(
            "// Per-page keyboard navigation",1)[0]
        self.assertIn("start_local_mutation(&state,LocalMutationKind::Move",cut)
        self.assertIn("start_local_mutation(&state,LocalMutationKind::Trash",trash)
        self.assertNotIn("transfer::move_local",cut)
        self.assertNotIn("gio_backend::move_to_trash",trash)

    def test_worker_is_cancelable_and_preserves_cut_state(self):
        worker = LOCAL.split("fn run_local_mutation(",1)[1].split(
            "fn start_local_mutation(",1)[0]
        delivery = LOCAL.split("fn start_local_mutation(",1)[1].split(
            "/// GIO supports local and remote URIs",1)[0]
        self.assertIn("std::thread::spawn(move ||",delivery)
        self.assertIn("cancel.load(Ordering::Acquire)",worker)
        self.assertIn("transfer::move_local(path,",worker)
        self.assertIn("gio_backend::move_to_trash(&uri)",worker)
        self.assertIn("p.is_move && &p.paths==original",delivery)
        self.assertIn("report.remaining.clone()",delivery)
        self.assertIn('browser.transfer_cancel.set_label("Annulla operazione")',delivery)
        self.assertIn("fn local_cut_never_overwrites_destination()",LOCAL)

    def test_local_navigation_avoids_blocking_stat_and_double_refresh(self):
        navigate=LOCAL.split("fn navigate(browser: &Browser,",1)[1].split(
            "fn navigation_icon_button(",1)[0]
        self.assertNotIn("target.is_dir()",navigate)
        self.assertIn("browser.search_revision.set(",navigate)
        self.assertIn("browser.suppress_search_refresh.set(true)",navigate)
        self.assertIn("watch_directory(browser)",navigate)
        self.assertIn("if state.suppress_search_refresh.get() {return;}",LOCAL)

    def test_monitor_generation_guards_local_and_remote(self):
        local=LOCAL.split("fn watch_directory(browser: &Browser)",1)[1].split(
            "fn repaint_volumes(",1)[0]
        remote=REMOTE.split("fn watch_current_directory(remote:&Remote)",1)[1].split(
            "/// Metadata is queried",1)[0]
        for code in (local,remote):
            self.assertIn("monitor_epoch",code)
            self.assertIn("cancel();",code)
            import re
            self.assertGreaterEqual(len(re.findall(r"!=\s*epoch", code)),2)
            self.assertIn("refresh_pending.set(false)",code)
        self.assertIn("root.connect_unrealize(move |_|",LOCAL)
        self.assertIn("root.connect_unrealize(move |_|",REMOTE)

    def test_remote_search_debounces_before_enumeration(self):
        search=REMOTE.split("state.search.connect_changed(move |_|",1)[1].split(
            "reload.connect_clicked",1)[0]
        self.assertIn("search_revision",search)
        self.assertIn("Duration::from_millis(160)",search)
        self.assertIn("revision.get()==ticket",search)


if __name__ == "__main__":
    unittest.main()
