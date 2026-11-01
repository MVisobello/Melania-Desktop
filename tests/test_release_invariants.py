"""Static invariants only: do not treat results as passing Rust tests."""
import pathlib
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

class ReleaseInvariants(unittest.TestCase):
    def test_drag_drop_targets_only_copy(self):
        source = (ROOT/'melania-files/src/main.rs').read_text()
        self.assertIn('gdk::DragAction::COPY', source)
        self.assertNotIn('source.set_actions(gdk::DragAction::MOVE)', source)
        self.assertNotIn('remove_confirmed_source(', source)

    def test_file_operations_prevent_overwrite(self):
        source = (ROOT/'melania-transfer/src/lib.rs').read_text()
        self.assertIn('create_new(true)', source)
        self.assertIn('RENAME_NOREPLACE', source)
        self.assertIn('plan_copy', source)

    def test_compositor_supports_wayland_data_and_layer_shell(self):
        source = (ROOT/'melania-compositor/src/main.rs').read_text()
        for token in ['delegate_data_device!', 'delegate_layer_shell!', 'impl WlrLayerShellHandler', 'ListeningSocket::bind_auto']:
            self.assertIn(token, source)

    def test_independent_gtk_shell_exists(self):
        source = (ROOT/'melania-panel/src/main.rs').read_text()
        for token in ['init_layer_shell()', 'fn start_window', 'fn quick_window', 'fn taskbar']:
            self.assertIn(token, source)

    def test_wayland_smoke_requires_real_gtk_window(self):
        smoke = (ROOT/'scripts/smoke-nested-wayland.sh').read_text()
        self.assertIn('melania-files before smoke test', smoke)
        self.assertIn('CLIENT_PRESENT=0', smoke)
        self.assertIn('GTK client did not map', smoke)
        self.assertIn('grep -Eiq', smoke)
        self.assertIn('exit 1', smoke)

    def test_ci_builds_gtk_client_for_compositor_smoke(self):
        ci = (ROOT/'.github/workflows/linux-verify.yml').read_text()
        self.assertIn('cargo build -p melania-compositor -p melania-control -p melania-files', ci)
        self.assertIn('libgtk-4-dev', ci)

    def test_files_core_is_integrated_and_build_targeted(self):
        workspace = (ROOT/'Cargo.toml').read_text()
        file_manifest = (ROOT/'melania-files/Cargo.toml').read_text()
        manager = (ROOT/'melania-files/src/main.rs').read_text()
        ci = (ROOT/'.github/workflows/linux-verify.yml').read_text()
        self.assertIn('"melania-files-core"', workspace)
        self.assertIn('melania-files-core = { path = "../melania-files-core" }', file_manifest)
        # The real GTK manager now enumerates through the shared GIO/GVfs
        # backend; the POSIX core listing remains available to CLI/tests.
        for token in ('gio_backend::list_local_entries', 'gtk::Notebook::new',
                      'sort.connect_selected_notify', 'repaint_favorites',
                      'transfer::rename_local', 'shared_pending.clone()'):
            self.assertIn(token, manager)
        backend=(ROOT/'melania-files/src/gio_backend.rs').read_text()
        self.assertIn('fn list_uri_bounded(',backend)
        self.assertIn('model::sort_entries(&mut entries,options)',backend)
        self.assertIn('cargo check -p melania-core', ci)
        self.assertIn('-p melania-files-core', ci)

    def test_rename_never_uses_overwriting_std_rename(self):
        transfer = (ROOT/'melania-transfer/src/lib.rs').read_text()
        self.assertIn('pub fn rename_local', transfer)
        self.assertIn('RENAME_NOREPLACE', transfer)
        self.assertIn('rename_is_atomic_and_preserves_destination', transfer)
        self.assertIn('rename_rejects_path_traversal', transfer)

    def test_bookmarks_and_sort_have_rust_tests(self):
        code = (ROOT/'melania-files-core/src/lib.rs').read_text()
        for needle in ('pub fn list_directory', 'pub struct Bookmarks',
                       'bookmarks_persist_and_deduplicate', 'sorts_by_size_desc',
                       'symlink_not_followed', 'max_entries'):
            self.assertIn(needle, code)

    def test_workspace_completeness(self):
        manifest = tomllib.loads((ROOT/'Cargo.toml').read_text())
        for name in manifest['workspace']['members']:
            self.assertTrue((ROOT/name/'Cargo.toml').is_file(), name)

if __name__ == '__main__':
    unittest.main()
