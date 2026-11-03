"""Nautilus-style dynamic tab interactions in the real Melania Files GTK UI.

These structural checks complement the Rust tab ordering unit tests and
the Wayland smoke test; they are not pixel-perfect GNOME visual tests.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
FILES=(ROOT/"melania-files/src/main.rs").read_text()
REMOTE=(ROOT/"melania-files/src/gio_remote.rs").read_text()
CSS=(ROOT/"melania-files/assets/style.css").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()
CI=(ROOT/".github/workflows/wayland-smoke.yml").read_text()


class FilesNautilusTabs(unittest.TestCase):
    def test_tab_rail_is_integrated_and_scrolled(self):
        for marker in (
            "aero_header.set_title_widget(Some(&no_title));",
            "tab_rail.append(&tab_scroll);",
            "tab_rail.append(&add_button);",
            "tab_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);",
            "reveal_active_files_tab(notebook,&strip_for_switch",
            "files-tab-active",
        ):
            self.assertIn(marker,FILES)
        self.assertIn("window_controls::install(&window, &aero_header)",FILES)

    def test_both_backends_share_the_same_tab_actions(self):
        self.assertIn('append_file_page(notebook,tab_strip,headers,window,text,child,"folder-symbolic",new_button)',FILES)
        self.assertIn('append_file_page(notebook,tab_strip,headers,window,title,child,"network-server-symbolic",new_button)',FILES)
        self.assertIn("browser.tab_title.set_tooltip_text(Some(&directory.display().to_string()))",FILES)
        self.assertIn("remote.title.set_tooltip_text(Some(&uri));",REMOTE)
        self.assertIn('place.as_deref().filter(|uri|uri.contains("://"))',FILES)

    def test_context_middle_click_and_drag_reorder_are_real_gtk_handlers(self):
        for marker in (
            "enum FilesTabAction { Select, New, Close, CloseOthers, CloseRight, MoveLeft, MoveRight }",
            "middle.set_button(2)",
            "right_click.set_button(3)",
            "menu.set_child(Some(&actions))",
            '("Chiudi altre schede",FilesTabAction::CloseOthers)',
            '("Chiudi schede a destra",FilesTabAction::CloseRight)',
            "let drag_source=gtk::DragSource::new();",
            "let drop=gtk::DropTarget::new(String::static_type(),gdk::DragAction::MOVE);",
            "tab_drop_destination(from,target,right_half)",
            "notebook.reorder_child(page,Some(destination));",
            "strip.reorder_child_after(&heading,previous.as_ref());",
        ):
            self.assertIn(marker,FILES)

    def test_tab_lifecycle_is_single_shared_implementation(self):
        start=FILES.split("fn close_files_tab(",1)[1].split("fn reorder_files_tab(",1)[0]
        self.assertIn("if notebook.n_pages()<=1{return;}",start)
        self.assertIn("notebook.remove_page(Some(index));",start)
        self.assertIn("headers.borrow_mut().retain(",start)
        self.assertIn("close_files_tab(&keys_notebook",FILES)

    def test_shortcuts_compatible_with_nautilus(self):
        keyboard=FILES.split("// GNOME/Nautilus tab shortcuts.",1)[1].split(
            "    window.add_controller(keys);",1)[0]
        for marker in ("gdk::Key::Page_Up","gdk::Key::Page_Down",
                       "gdk::Key::Home","gdk::Key::End","gdk::Key::Tab",
                       "gdk::Key::w","gdk::Key::t","to_unicode()",
                       "reorder_files_tab("):
            self.assertIn(marker,keyboard)

    def test_actual_wayland_session_captures_multiple_tabs(self):
        for marker in ('export MELANIA_SMOKE_FILES_TABS=1',
                       'xdotool key --clearmodifiers ctrl+t',
                       'xdotool key --clearmodifiers ctrl+w',
                       'MELANIA_FILES_TAB_COUNT=',
                       'scrot -z -o reports/nested-wayland-files-tabs.png'):
            self.assertIn(marker,SMOKE)
        self.assertIn('reports/nested-wayland-files-tabs.png',CI)
        self.assertIn('eprintln!("MELANIA_FILES_TAB_COUNT={}"',FILES)

    def test_graphic_is_nautilus_like_in_all_three_themes(self):
        for marker in ("files-tab-rail","files-top-tab.files-tab-active",
                       "files-top-tab .files-tab-close",
                       "files-top-tab-select:focus-visible",
                       "popover.files-tab-menu > contents",
                       "window.melania-theme-light .files-top-tab",
                       "window.melania-theme-dark .files-top-tab",
                       "window.melania-theme-aero .files-top-tab"):
            self.assertIn(marker,CSS)
        self.assertIn("border-radius:10px 10px 0 0",CSS)
        self.assertIn("opacity:0.15",CSS)


if __name__=="__main__":
    unittest.main()
