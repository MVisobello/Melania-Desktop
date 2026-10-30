"""Static integration checks only: cannot substitute Rust compiler/GTK integration tests."""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = (ROOT / 'melania-files-core/src/nautilus_features.rs').read_text()
UI = (ROOT / 'melania-files/src/main.rs').read_text()

class NautilusFoundationStructure(unittest.TestCase):
    def test_core_library_exported(self):
        self.assertIn('pub mod nautilus_features;', (ROOT/'melania-files-core/src/lib.rs').read_text())
    def test_recursive_search_is_bounded_and_not_following_symlinks(self):
        for token in ('max_results', 'max_directories', 'symlink_metadata', 'pub fn search_tree'):
            self.assertIn(token, CORE)
    def test_properties_do_not_follow_symlinks(self):
        for token in ('pub fn properties', 'symlink_target', 'symlink_metadata'):
            self.assertIn(token, CORE)
    def test_batch_rename_plan_has_no_writes(self):
        before_tests = CORE.split('#[cfg(test)]', 1)[0]
        block = before_tests.split('pub fn batch_rename_plan(',1)[1].split('pub fn create_from_template',1)[0]
        self.assertNotIn('fs::rename(', block)
        self.assertIn('symlink_metadata(&target)', block)
    def test_templates_use_exclusive_creation(self):
        self.assertIn('pub fn create_from_template', CORE)
        self.assertIn('create_new(true)', CORE)
        self.assertIn('Da modello', UI)
    def test_context_menu_uses_safe_trash_action(self):
        for token in ('gtk::Popover::new()', 'gdk::BUTTON_SECONDARY',
                      'state.trash_action.emit_clicked()', 'show_properties(&target)'):
            self.assertIn(token, UI)
    def test_gio_mount_and_watch_handlers(self):
        for token in ('connect_mount_added', 'connect_mount_removed', 'monitor_directory(',
                      'fn repaint_volumes', 'fn watch_directory'):
            self.assertIn(token, UI)
    def test_shortcuts_and_editable_location(self):
        for token in ('gdk::Key::l', 'gdk::Key::h', 'gdk::Key::F2', 'location.connect_activate'):
            self.assertIn(token, UI)
    def test_dragdrop_kept_copy_only(self):
        self.assertIn('gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)', UI)
        self.assertIn('source.set_actions(gdk::DragAction::COPY)', UI)
    def test_no_system_theme_change(self):
        self.assertNotIn('gsettings set', UI)

if __name__ == '__main__': unittest.main()
