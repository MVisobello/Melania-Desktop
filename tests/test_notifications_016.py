"""Offline repository checks. These do NOT compile Rust or test D-Bus/Wayland."""
from pathlib import Path
import tomllib
import unittest
ROOT=Path(__file__).resolve().parent.parent

class NotificationIntegration016(unittest.TestCase):
    def test_workspace_wires_both_notifications_crates(self):
        manifest=tomllib.loads((ROOT/'Cargo.toml').read_text())
        self.assertIn('melania-notify-core',manifest['workspace']['members'])
        self.assertIn('melania-notifyd',manifest['workspace']['members'])
        self.assertIn('melania-notify-core', (ROOT/'melania-panel/Cargo.toml').read_text())

    def test_dbus_service_is_freedesktop_compatible_at_source_level(self):
        code=(ROOT/'melania-notifyd/src/main.rs').read_text()
        for token in ('org.freedesktop.Notifications',
                      '/org/freedesktop/Notifications', 'fn notify(',
                      'fn close_notification(', 'fn get_capabilities(',
                      'fn get_server_information(', 'notification_closed('):
            self.assertIn(token,code)

    def test_notification_storage_is_bounded_and_atomic(self):
        code=(ROOT/'melania-notify-core/src/lib.rs').read_text()
        for token in ('MAX_HISTORY: usize = 100', 'MAX_TEXT: usize = 4096',
                      'entries.truncate(MAX_HISTORY)', 'create_new(true)',
                      'fs::rename(&temp, path)', '0o600', 'snapshot_path()'):
            self.assertIn(token,code)

    def test_notification_panel_is_not_an_xdg_toplevel(self):
        code=(ROOT/'melania-panel/src/main.rs').read_text()
        self.assertIn('fn notifications_window',code)
        self.assertIn('Layer::Overlay',code)
        self.assertIn('read_snapshot(',code)
        self.assertIn('let notifications=notifications_window(app);',code)

    def test_ipc_commands_must_wait_for_writer_eof(self):
        protocol=(ROOT/'melania-control/src/lib.rs').read_text()
        compositor=(ROOT/'melania-compositor/src/main.rs').read_text()
        for key in ('shutdown(std::net::Shutdown::Write)',
                    'pub fn read_command', 'rejects_multiple_commands_in_one_connection',
                    'fragmented_ipc_requires_eof'):
            self.assertIn(key,protocol)
        self.assertIn('read_command(&mut client)',compositor)
        self.assertNotIn('client.read(&mut input)',compositor)

    def test_notification_bus_test_is_isolated(self):
        script=(ROOT/'scripts/smoke-notifications-dbus.sh').read_text()
        self.assertIn('dbus-run-session -- "$0" --inside',script)
        self.assertIn('notify-send -a MelaniaSmoke',script)
        self.assertIn('Melania test',script)
        workflow=(ROOT/'.github/workflows/linux-verify.yml').read_text()
        self.assertIn('rust-notifications:',workflow)
        self.assertIn('./scripts/smoke-notifications-dbus.sh',workflow)

if __name__=='__main__': unittest.main()
