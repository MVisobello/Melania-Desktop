"""Regression guards for real user-triggered GTK/GVfs mount operations.

Structural checks supplement (do not replace) Rust compilation and nested
Wayland GUI verification. Network authentication requires a real server.
"""
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[1]
MOUNT=(ROOT/'melania-files/src/gio_mount.rs').read_text()
REMOTE=(ROOT/'melania-files/src/gio_remote.rs').read_text()
MAIN=(ROOT/'melania-files/src/main.rs').read_text()


class GVfsMountControls(unittest.TestCase):
    def test_native_authentication_with_explicit_connection_and_cancel(self):
        self.assertIn('gtk::MountOperation::new(parent)', MOUNT)
        self.assertIn('.mount_enclosing_volume(', MOUNT)
        self.assertIn('gio::MountMountFlags::NONE', MOUNT)
        self.assertIn('gio_mount::connect_uri(&uri,window.as_ref()', REMOTE)
        self.assertIn('connect.connect_clicked(move |_| {', REMOTE)
        self.assertIn('cancellable.cancel()', REMOTE)
        self.assertNotIn('Command::new(', MOUNT)
        self.assertNotIn('.password()', MOUNT)

    def test_safe_removable_volume_controls_and_explicit_confirmation(self):
        self.assertIn('gio::VolumeMonitor::get().volumes()',MAIN)
        self.assertIn('volume.get_mount().is_some()', MAIN)
        self.assertIn('gio_mount::mount_volume(&selected', MAIN)
        self.assertIn('gio_mount::confirm_mount_action(', MAIN)
        self.assertIn('gio_mount::unmount(&selected', MAIN)
        self.assertIn('gio_mount::eject(&selected', MAIN)
        self.assertIn('mount.can_unmount()',MAIN)
        self.assertIn('mount.can_eject()',MAIN)
        self.assertIn('gio::MountUnmountFlags::NONE',MOUNT)
        self.assertNotIn('MountUnmountFlags::FORCE',MOUNT)
        self.assertNotIn('rm -rf',MOUNT)

    def test_hotplug_and_keeping_root_unchanged(self):
        for name in ('connect_volume_added', 'connect_volume_removed',
                     'connect_volume_changed', 'connect_mount_added',
                     'connect_mount_removed'):
            self.assertIn(name,MAIN)
        self.assertIn('root.uri()!="file:///"',MAIN)
        self.assertIn('dialog.add_button("Annulla"',MOUNT)
        self.assertIn('response==gtk::ResponseType::Accept',MOUNT)


if __name__=='__main__':
    unittest.main()
