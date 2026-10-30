"""Release invariants, not Rust runtime tests. Real cargo/Wayland checks remain mandatory."""
from pathlib import Path
from unittest import TestCase
import configparser
import tomllib

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "melania-desktop-surface"
SRC = (MODULE / "src/main.rs").read_text()

class DesktopSurfaceIntegrationTests(TestCase):
    def test_module_is_part_of_cargo_workspace(self):
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertIn("melania-desktop-surface", workspace["workspace"]["members"])
        manifest = tomllib.loads((MODULE / "Cargo.toml").read_text())
        self.assertIn("gtk", manifest["dependencies"])
        self.assertIn("gtk4-layer-shell", manifest["dependencies"])
        self.assertIn("melania-transfer", manifest["dependencies"])

    def test_desktop_is_a_background_layer_below_windows(self):
        self.assertIn("window.set_layer(Layer::Background)", SRC)
        self.assertIn("window.set_exclusive_zone(0)", SRC)
        compositor = (ROOT / "melania-compositor/src/main.rs").read_text()
        self.assertIn("WlrLayer::Background", compositor)
        self.assertIn("WlrLayer::Top", compositor)

    def test_drag_drop_is_copy_only(self):
        self.assertIn("source.set_actions(gdk::DragAction::COPY)", SRC)
        self.assertIn("gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)", SRC)
        self.assertIn("melania_transfer::copy_sources(&sources, &dir_after_drop)", SRC)
        self.assertNotIn("gdk::DragAction::MOVE", SRC)
        self.assertNotIn("remove_file", SRC)
        self.assertNotIn("remove_dir_all", SRC)

    def test_uses_localized_xdg_desktop_and_monitor(self):
        self.assertIn('arg("DESKTOP")', SRC)
        self.assertIn("monitor_directory", SRC)
        self.assertIn("monitor.connect_changed", SRC)

    def test_build_ci_and_nested_session_include_module(self):
        ci = (ROOT / ".github/workflows/linux-verify.yml").read_text()
        self.assertIn("cargo check -p melania-files", ci)
        self.assertIn("-p melania-desktop-surface", ci)
        run = (ROOT / "scripts/run-melania-v018-nested.sh").read_text()
        self.assertIn('target/debug/melania-desktop-surface', run)

    def test_desktop_entry_is_valid(self):
        cp = configparser.ConfigParser(interpolation=None)
        cp.read(ROOT / "integration/org.melaniadesktop.Desktop.desktop")
        self.assertEqual(cp["Desktop Entry"]["Exec"], "melania-desktop-surface")
        self.assertEqual(cp["Desktop Entry"]["NoDisplay"], "true")

    def test_wallpaper_is_png_at_target_resolution(self):
        from struct import unpack
        with (MODULE / "assets/wallpaper.png").open("rb") as fh:
            png = fh.read(24)
        self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(unpack(">II", png[16:24]), (1600, 900))
