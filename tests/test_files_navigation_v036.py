"""GTK source checks; no claims of compilation or runtime validation."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/'melania-files/src/main.rs').read_text(encoding='utf-8')
CSS=(ROOT/'melania-files/assets/style.css').read_text(encoding='utf-8')

class Files036(unittest.TestCase):
    def test_title_belongs_to_each_tab(self):
        for text in ('tab_title: Label', 'browser.tab_title.set_text(&caption)',
                     'fn file_tab(start_dir: PathBuf, shared_pending:',
                     'let child = file_tab(path, pending, text.clone(), open_remote)'):
            self.assertIn(text, SOURCE)

    def test_local_and_gvfs_use_same_tab_rail(self):
        for marker in ('fn append_remote_tab(', 'append_file_page(notebook,tab_strip,headers,window',
                       'mod gio_backend;', 'mod gio_remote;', 'network_button.connect_clicked'):
            self.assertIn(marker, SOURCE)

    def test_single_click_selects_double_click_opens(self):
        click=SOURCE[SOURCE.index('button.connect_clicked(move |_| {'):SOURCE.index('let activation = gtk::GestureClick::new()')]
        self.assertIn('state.selected_paths.borrow_mut()', click)
        self.assertNotIn('navigate(', click)
        self.assertIn('if count != 2 { return; }', SOURCE)
        self.assertIn('navigate(&active_browser, active_path.clone(), true)', SOURCE)
        self.assertIn('gio::AppInfo::launch_default_for_uri', SOURCE)

    def test_filtered_selection_is_pruned(self):
        self.assertIn('browser.selected_paths.borrow_mut().retain(|path| visible.contains(path))', SOURCE)
        self.assertIn('browser.selected.borrow_mut()', SOURCE)
        self.assertIn('browser.highlighted.borrow_mut().take();', SOURCE)
        self.assertIn('files-tile-selected', CSS)

    def test_navigation_shortcuts_stay_per_tab(self):
        self.assertIn('root.add_controller(key_nav)', SOURCE)
        for expr in ('gdk::Key::Left','gdk::Key::Right','gdk::Key::Up','gdk::Key::F5','gdk::Key::f'):
            self.assertIn(expr, SOURCE)
        self.assertIn('back: Rc::new(RefCell::new(Vec::new()))', SOURCE)
        self.assertIn('forward: Rc::new(RefCell::new(Vec::new()))', SOURCE)

    def test_dnd_stays_copy_only(self):
        self.assertIn('source.set_actions(gdk::DragAction::COPY)', SOURCE)
        self.assertIn('gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)', SOURCE)
        self.assertNotIn('gdk::DragAction::MOVE)', SOURCE)
        self.assertIn('gio::File::for_path(&path).trash(None::<&gio::Cancellable>)', SOURCE)

if __name__=='__main__': unittest.main()
