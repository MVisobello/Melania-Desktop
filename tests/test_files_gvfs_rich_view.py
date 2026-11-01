"""Structure and security guards for the GIO remote-files rich view.

These checks do not substitute compiling Rust/GTK or a live GVfs server test.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
UI=(ROOT/"melania-files/src/gio_remote.rs").read_text()
GIO=(ROOT/"melania-files/src/gio_backend.rs").read_text()

class GVfsRichView(unittest.TestCase):
    def test_remote_sort_hidden_and_metadata_columns(self):
        for snippet in ("gio_backend::list_uri_sorted(","model::SortBy::from_index",
                        'gtk::DropDown::from_strings(', 'CheckButton::with_label("Nascosti")',
                        'key==gdk::Key::h', 'format_size(item.size)',
                        'format_modified(item.modified)', '&item.content_type'):
            self.assertIn(snippet,UI)
        self.assertIn("sort_items(&mut listing.items,sort,descending)",GIO)
        self.assertIn("if a.directory!=b.directory",GIO)

    def test_properties_metadata_never_recursive_or_on_gtk_thread(self):
        self.assertIn("std::thread::spawn(move || {\n        let result=gio_backend::query_details(&uri)",UI)
        self.assertIn("gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS",GIO)
        self.assertIn("access::can-read",GIO)
        self.assertIn("access::can-write",GIO)
        self.assertIn('properties.connect_clicked(move |_|{',UI)
        self.assertNotIn("fs::read_dir(",GIO)

    def test_monitor_debounced_and_dropped_with_tab(self):
        self.assertIn("fn watch_current_directory(remote:&Remote)",UI)
        self.assertIn("gio::FileMonitorFlags::NONE",UI)
        self.assertIn("refresh_pending.replace(true)",UI)
        self.assertIn("Duration::from_millis(260)",UI)
        self.assertIn("root.connect_unrealize(move |_| {",UI)
        self.assertIn("monitor.cancel()",UI)


if __name__=="__main__":
    unittest.main()
