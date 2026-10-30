"""Static integration checks only; Rust/GTK runtime tests run in CI with cargo."""
from pathlib import Path
from unittest import TestCase
import tomllib

ROOT = Path(__file__).resolve().parents[1]
CORE = (ROOT / "melania-desktop-core/src/lib.rs").read_text()
SURFACE = (ROOT / "melania-desktop-surface/src/main.rs").read_text()
PANEL = (ROOT / "melania-panel/src/main.rs").read_text()
CSS = (ROOT / "melania-desktop-surface/assets/desktop.css").read_text()
CI = (ROOT / ".github/workflows/linux-verify.yml").read_text()

class Desktop019Integration(TestCase):
    def test_core_is_member_and_surface_dependency(self):
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
        surface = tomllib.loads((ROOT / "melania-desktop-surface/Cargo.toml").read_text())
        self.assertIn("melania-desktop-core", workspace["workspace"]["members"])
        self.assertIn("melania-desktop-core", surface["dependencies"])
        self.assertIn("-p melania-desktop-core", CI)

    def test_click_selection_and_double_click_open(self):
        self.assertIn("click.connect_pressed", SURFACE)
        self.assertIn("if count == 2", SURFACE)
        self.assertIn("CONTROL_MASK", SURFACE)
        self.assertIn("selection.paths().take(256)", SURFACE)
        self.assertIn("melania-icon-selected", CSS)

    def test_safe_desktop_folder_creation(self):
        self.assertIn("fs::create_dir(&candidate)", CORE)
        self.assertIn("ErrorKind::AlreadyExists", CORE)
        self.assertNotIn("fs::remove_file", CORE)
        self.assertNotIn("fs::remove_dir_all", CORE.split("#[cfg(test)]")[0])
        self.assertIn("simultaneous_creation_never_collides", CORE)
        self.assertIn("makes_folders_without_overwriting_files", CORE)
        self.assertIn("create_unique_folder(&create_dir)", SURFACE)

    def test_folder_and_desktop_drops_are_copy_only(self):
        self.assertIn("gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)", SURFACE)
        self.assertIn("copy_sources(&sources, &folder)", SURFACE)
        self.assertIn("copy_sources(&sources, &dir_after_drop)", SURFACE)
        self.assertIn("source.set_actions(gdk::DragAction::COPY)", SURFACE)
        self.assertNotIn("DragAction::MOVE", SURFACE)
        self.assertNotIn("remove_confirmed_source", SURFACE)
        self.assertIn("melania-folder-drop-ready", CSS)

    def test_context_menu_has_real_operations(self):
        self.assertIn("gtk::Popover::new()", SURFACE)
        self.assertIn('Button::with_label("Nuova cartella")', SURFACE)
        self.assertIn('Button::with_label("Aggiorna desktop")', SURFACE)
        self.assertIn('Button::with_label("Apri Melania Files")', SURFACE)

    def test_localized_start_menu_shortcuts(self):
        self.assertIn('arg(kind).output()', PANEL)
        self.assertIn('"Documenti", "Documents"', PANEL)
        self.assertIn('"Immagini", "Pictures"', PANEL)
        self.assertNotIn('std::env::var(variable)', PANEL)

    def test_core_has_tests_for_sort_and_symlinks(self):
        self.assertIn("sorting_is_stable_and_hidden_files_are_hidden", CORE)
        self.assertIn("symlink_is_not_treated_as_directory", CORE)
        self.assertIn("select_replaces_and_control_click_toggles", CORE)
        self.assertIn("items.binary_search_by", CORE)
