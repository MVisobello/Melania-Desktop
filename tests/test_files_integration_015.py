"""Static regression checks for copy-only GTK integration.

These are not Rust compiler/runtime tests. Expectations follow the new
cancellable background transfer engine introduced in v0.38.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent
SOURCE = (ROOT / 'melania-files/src/main.rs').read_text(encoding='utf-8')
BG = (ROOT / 'melania-transfer/src/background.rs').read_text(encoding='utf-8')

class FilesIntegration015(unittest.TestCase):
    def test_clipboard_interoperable_filelists(self):
        self.assertIn('display.clipboard().set_content(Some(&provider))', SOURCE)
        self.assertIn('display.clipboard().read_value_async(', SOURCE)
        self.assertIn('gdk::FileList::static_type()', SOURCE)
        self.assertIn('paths.iter().map(gio::File::for_path)', SOURCE)

    def test_cut_is_local_atomic_no_replace(self):
        self.assertIn('pending.filter(|p|p.is_move)', SOURCE)
        self.assertIn('transfer::move_local(&path,&target)', SOURCE)
        self.assertIn('source.set_actions(gdk::DragAction::COPY)', SOURCE)
        self.assertIn('gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)', SOURCE)
        self.assertNotIn('gdk::DragAction::MOVE)', SOURCE)
        self.assertNotIn('remove_confirmed_source(', SOURCE)

    def test_trash_remains_recoverable(self):
        self.assertIn('gio::File::for_path(&path).trash(None::<&gio::Cancellable>)', SOURCE)
        self.assertIn('trash:///', SOURCE)
        self.assertNotIn('fs::remove_file(', SOURCE)
        self.assertNotIn('fs::remove_dir_all(', SOURCE)

    def test_theme_and_localized_folders(self):
        self.assertIn('fn xdg_user_directory(', SOURCE)
        for k in ('"DOCUMENTS"', '"DESKTOP"', '"PICTURES"', '"DOWNLOAD"', '"MUSIC"', '"VIDEOS"'):
            self.assertIn(k, SOURCE)
        self.assertIn('Image::from_icon_name(entry_icon(&path, folder))', SOURCE)

    def test_background_drop_only_accepts_validated_copy(self):
        drop = SOURCE[SOURCE.index('fn attach_drop('):SOURCE.index('fn show_properties(')]
        self.assertIn('transfer::plan_copy(&sources, &target)', drop)
        self.assertIn('start_background_copy(&browser, sources, target)', drop)
        self.assertIn('gdk::DragAction::COPY', drop)
        self.assertIn('false', drop)
        self.assertIn('true', drop)
        self.assertIn('fs::OpenOptions::new().write(true).create_new(true)', BG)

if __name__ == '__main__':
    unittest.main()
