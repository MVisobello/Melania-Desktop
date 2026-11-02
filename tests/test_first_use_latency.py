"""First-use GTK responsiveness regressions for Start and Files."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text()
FILES=(ROOT/"melania-files/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class FirstUseLatency(unittest.TestCase):
    def test_start_recent_apps_share_the_one_catalog_scan(self):
        start=PANEL[PANEL.index("fn start_window("):PANEL.index("fn quick_window(")]
        # One Gio full-app enumeration for the entire Start surface: recent
        # entries reuse the exact installed catalog rather than doing I/O.
        self.assertEqual(start.count("gio::AppInfo::all()"),1)
        self.assertIn("let installed_cache:Rc<RefCell<Vec<gio::AppInfo>>>",start)
        self.assertIn("let installed=installed_for_recent.borrow();",start)
        self.assertIn("refresh_recent.borrow_mut()=Some(refresh_recents.clone())",start)
        self.assertIn("recents_after_catalog.borrow().as_ref()",start)
        self.assertIn('Some("opacity")',start)
        self.assertIn("recent_list.set_visible(count>0);",start)

    def test_search_enter_does_not_hold_shared_refcell_borrow_through_click(self):
        start=PANEL[PANEL.index("fn start_window("):PANEL.index("fn quick_window(")]
        self.assertIn("let first=searchable_for_enter.borrow().iter()",start)
        self.assertIn(".map(|(button,_,_)|button.clone());",start)
        self.assertIn("if let Some(button)=first {button.emit_clicked();}",start)

    def test_files_chrome_first_and_full_browser_remains_functional(self):
        build=FILES[FILES.index("fn build_ui(app: &Application)"):FILES.index("/// Prefer Melania's freedesktop icon theme")]
        self.assertIn('Label::new(Some("Caricamento cartella…"))',build)
        self.assertIn("window.present();",build)
        self.assertIn("timeout_add_local_once(Duration::from_millis(55)",build)
        self.assertLess(build.index("window.present();"),build.index("append_browser_tab(&first_tabs"))
        self.assertIn("first_tabs.remove_page(Some(0));",build)
        self.assertIn("if let Some(uri)=requested_uri {first_remote(uri);}",build)
        self.assertIn('std::env::var("MELANIA_SMOKE_GIO_FILE_URI")',build)
        self.assertIn("MELANIA_FILES_FIRST_WINDOW_MS=",build)
        self.assertIn("MELANIA_FILES_FIRST_TAB_MS=",build)

    def test_real_nested_gui_confirms_order(self):
        self.assertIn('window<tab<listing',SMOKE)
        self.assertIn('MELANIA_FILES_LISTING_READY count=',SMOKE)

if __name__=="__main__":
    unittest.main()
