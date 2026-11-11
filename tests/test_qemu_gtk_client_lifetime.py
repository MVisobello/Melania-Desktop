"""The real GTK4 input client must survive the bounded DRM verifier.

This is a source regression for process lifetime ordering; only the full
isolated QEMU USB-to-GTK-to-CRTC run proves the functional behavior.
"""
from pathlib import Path
import re
import unittest

GUEST = (Path(__file__).resolve().parents[1] /
         "scripts/melania-qemu-native-guest.sh").read_text()


class GtkUsbInputLifetime(unittest.TestCase):
    def test_real_gtk_client_outlives_drm_verifier_but_not_smithay(self):
        # Earlier GTK smoke sessions intentionally use shorter timeouts.
        # Inspect only the real second-frame USB-search session, so this
        # contract checks the processes that actually share a lifetime.
        start = GUEST.index('MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM')
        end = GUEST.index("grep -Fqx 'MELANIA_QEMU_GTK_SECOND_REAL_FRAME_REQUESTED=true'", start)
        session = GUEST[start:end]
        def session_timeout(command):
            match = re.search(
                r'timeout --signal=TERM --kill-after=3s (\d+)s\s+(?:\\\s+)?'
                + re.escape(command), session)
            self.assertIsNotNone(match, command)
            return int(match.group(1))

        client = session_timeout('"$gtk_panel"')
        drm = session_timeout('"$continuous_kms"')
        server = session_timeout('"$binary" --experimental-wayland-seat')
        self.assertLess(drm, client, "GTK4 USB recipient must outlive DRM")
        self.assertLess(client, server, "Smithay must own GTK client lifecycle")
        self.assertLessEqual(server, 60, "keep the test bounded")

    def test_real_gtk_process_receives_its_loader_and_wayland_environment(self):
        # The earlier QEMU failure was a shell continuation broken by a
        # comment between the final assignment and 'timeout'. The GTK
        # executable then ran WITHOUT LD_LIBRARY_PATH and failed to load
        # libgtk4-layer-shell.so.0, before any pixel/USB assertions.
        start = GUEST.index("LD_LIBRARY_PATH=/mnt/melania-native \\\n"
                            "MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native", 
                            GUEST.index("MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM"))
        end = GUEST.index('> "$gtk2_client_log" 2>&1 &', start)
        launch = GUEST[start:end]
        self.assertIn('WAYLAND_DISPLAY="$gtk2_socket" MELANIA_PROFILE_START=1', launch)
        self.assertIn('timeout --signal=TERM --kill-after=3s 52s "$gtk_panel"', launch)
        # No comments or blank commands may interrupt any continued
        # environment assignment. Check every physical continuation line.
        lines = launch.splitlines()
        for index, line in enumerate(lines[:-1]):
            if line.rstrip().endswith("\\\\"):
                self.assertTrue(lines[index + 1].strip(), "empty continued line")
                self.assertFalse(lines[index + 1].lstrip().startswith("#"),
                                 "comment detached GTK environment")
        self.assertRegex(
            launch,
            r'WAYLAND_DISPLAY="\$gtk2_socket" MELANIA_PROFILE_START=1 \\\n'
            r'\s*timeout --signal=TERM --kill-after=3s 52s "\$gtk_panel"',
        )

    def test_real_usb_search_and_crtc_assertions_are_not_bypassed(self):
        for marker in (
            "MELANIA_QEMU_GTK_START_REAL_SEARCH_TYPED=true",
            "MELANIA_NATIVE_GTK_START_REAL_KEYBOARD_FOCUS=true",
            "MELANIA_NATIVE_GTK_SUPER_KEY_RELEASED=true",
            "MELANIA_RUST_REAL_GTK_START_OVERLAY_KMS_PASS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true",
        ):
            self.assertIn("grep -Fqx '" + marker + "'", GUEST)


if __name__ == "__main__":
    unittest.main()
