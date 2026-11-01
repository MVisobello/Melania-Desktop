#!/usr/bin/env python3
"""Melania Files GTK worker architecture regression guards."""
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"melania-files/src/main.rs").read_text(encoding="utf-8")
MODEL=(ROOT/"melania-files-core/src/lib.rs").read_text(encoding="utf-8")

class AsyncFilesystemListing(unittest.TestCase):
    def test_one_worker_per_tab_not_one_per_keystroke(self):
        self.assertIn("listing_generation: Rc<Cell<u64>>",SOURCE)
        self.assertIn("listing_running: Rc<Cell<bool>>",SOURCE)
        self.assertIn("listing_pending: Rc<RefCell<Option<ListingRequest>>>",SOURCE)
        self.assertIn("if browser.listing_running.get() {return;}",SOURCE)
        self.assertIn("browser.listing_pending.borrow_mut().take()",SOURCE)
        self.assertIn("Some(ListingRequest{",SOURCE)

    def test_no_gtk_io_and_generation_guard(self):
        self.assertIn("std::thread::spawn(move || {",SOURCE)
        self.assertIn("gio_backend::list_local_entries(&request.directory,&request.query,",SOURCE)
        self.assertIn("request.options,&request.cancelled)",SOURCE)
        self.assertIn("previous.store(true,Ordering::Release)",SOURCE)
        self.assertIn("receiver.try_recv()",SOURCE)
        self.assertIn("tab.listing_generation.get()==request.generation",SOURCE)
        self.assertIn("render_listing(&tab,&request.directory,entries)",SOURCE)
        self.assertIn("start_listing_worker(&tab)",SOURCE)
        refresh=SOURCE.split("fn refresh(browser: &Browser) {",1)[1].split("fn render_listing(",1)[0]
        self.assertNotIn("model::list_directory",refresh)
        self.assertIn('browser.status.set_text("Caricamento…")',refresh)

    def test_stale_items_cannot_be_acted_on_before_new_folder_loaded(self):
        source=SOURCE.split("fn refresh(browser: &Browser) {",1)[1].split("fn render_listing(",1)[0]
        for pattern in [
            "browser.tile_buttons.borrow_mut().clear();",
            "browser.visible_paths.borrow_mut().clear();",
            "browser.selected_paths.borrow_mut().clear();",
            "*browser.selected.borrow_mut()=None;",
            "*browser.selection_anchor.borrow_mut()=None;",
        ]:
            self.assertIn(pattern,source)
        self.assertIn("browser.selected_paths.borrow_mut().retain(|path| visible.contains(path))",SOURCE)
        self.assertIn("gio::File::for_path(dir).monitor_directory(",SOURCE)
        self.assertIn("pub fn list_directory(dir: &Path",MODEL)

if __name__=="__main__":
    unittest.main()
