"""Source-level release invariants ONLY: Rust/GTK and DBus need Linux build."""
from pathlib import Path
import tomllib
import unittest
ROOT = Path(__file__).resolve().parent.parent

class NotificationControls017(unittest.TestCase):
    def test_workspace_wires_control_binary(self):
        manifest = tomllib.loads((ROOT/'Cargo.toml').read_text())
        self.assertIn('melania-notifyctl', manifest['workspace']['members'])
        self.assertGreaterEqual(tuple(map(int, manifest['workspace']['package']['version'].split('.'))), (0, 17, 0))

    def test_notification_limit_is_compatible_with_snapshot_size(self):
        core = (ROOT/'melania-notify-core/src/lib.rs').read_text()
        for token in ('MAX_HISTORY: usize = 100', 'MAX_TEXT: usize = 4096',
                      'MAX_APP: usize = 128', 'MAX_SUMMARY: usize = 256',
                      'pub unread: bool', 'fn unread_count(', 'fn mark_all_read(',
                      'boundaries_keep_snapshot_under_size_limit'):
            self.assertIn(token, core)

    def test_acknowledgment_does_not_emit_closed_notification(self):
        svc = (ROOT/'melania-notifyd/src/main.rs').read_text()
        self.assertIn('org.melaniadesktop.NotificationCenter1', svc)
        self.assertIn('fn mark_all_read(', svc)
        self.assertIn('fn unread_count(', svc)
        self.assertIn('org.freedesktop.Notifications',svc)
        self.assertIn('notification_closed(',svc)

    def test_actionable_panel_and_badge(self):
        panel = (ROOT/'melania-panel/src/main.rs').read_text()
        for needle in ('Segna lette', 'melania-notifyctl', 'melania-close-notice',
                       'melania-notice-unread', 'n.unread', 'snapshot.notifications',
                       'close'):
            self.assertIn(needle, panel)

    def test_nested_runner_builds_controls_and_uses_private_bus(self):
        runner = (ROOT/'scripts/run-melania-v016-nested.sh').read_text()
        self.assertIn('dbus-run-session',runner)
        self.assertIn('-p melania-notifyctl',runner)
        self.assertIn('export PATH="$ROOT/target/debug:$PATH"',runner)

    def test_smoke_exercises_unread_and_close(self):
        smoke = (ROOT/'scripts/smoke-notifications-dbus.sh').read_text()
        for needle in ('melania-notifyctl unread', 'melania-notifyctl mark-read',
                       'melania-notifyctl close', 'dbus-run-session'):
            self.assertIn(needle, smoke)

if __name__ == '__main__': unittest.main()
