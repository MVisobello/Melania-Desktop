"""Keep original Melania identity and existing desktop features intact.

These static regression guards complement, but do not replace, GTK/Wayland
runtime tests in GitHub Actions.
"""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


class AeroContinuity(unittest.TestCase):
    def source(self, name):
        return (ROOT / name).read_text(encoding="utf-8")

    def test_original_melania_identity_is_preserved(self):
        for path in [
            "branding/icons/melania-128.png",
            "branding/icons/melania-start-64.png",
            "branding/icons/melania-symbol.svg",
            "branding/Melania-Aero-Classic/index.theme",
        ]:
            self.assertTrue((ROOT / path).is_file(), path)

    def test_aero_glass_and_native_switch_controls_remain(self):
        css = self.source("melania-settings/assets/style.css")
        rust = self.source("melania-settings/src/main.rs")
        self.assertIn("settings-titlebar", css)
        self.assertIn("settings-switch > slider", css)
        self.assertIn('toggle.set_valign(gtk::Align::Center)', rust)
        self.assertIn('gtk-enable-animations', rust)

    def test_classic_start_files_and_quick_settings_remain(self):
        panel = self.source("melania-panel/src/main.rs")
        for label in [
            'fn start_window(', 'fn quick_window(', 'fn notifications_window(',
            'fn calendar_window(', 'fn taskbar(', 'fn running_window_strip(',
            'fn workspace_strip(', 'melania-files', 'WindowCommand::Close',
            'gdk::DragAction::COPY',
        ]:
            self.assertIn(label, panel)

    def test_desktop_compositor_not_replaced(self):
        compositor = self.source("melania-compositor/src/main.rs")
        for required in [
            "smithay", "Workspaces::new(", "snap_windows",
            "finish_interaction", "WinitEvent::Input", "DataDeviceState",
        ]:
            self.assertIn(required, compositor)

    def test_files_safety_features_still_present(self):
        files = self.source("melania-files/src/main.rs")
        for required in [
            "melania_transfer as transfer", "nautilus_features",
            "gdk::DragAction::COPY", "PendingTransfer", "setup_melania_icon_theme",
        ]:
            self.assertIn(required, files)

    def test_productivity_features_are_opt_in(self):
        config = self.source("melania-config/src/lib.rs")
        self.assertIn("group_windows: false", config)
        self.assertIn("reduce_motion: false", config)
        self.assertIn("high_contrast: false", config)
        panel = self.source("melania-panel/src/main.rs")
        self.assertIn("group_enabled", panel)
        self.assertIn("melania-workspace-overflow", panel)


if __name__ == "__main__":
    unittest.main()
