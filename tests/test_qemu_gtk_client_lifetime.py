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
        def timeout_for(command):
            match = re.search(
                r'timeout --signal=TERM --kill-after=3s (\\d+)s\\s+'
                + re.escape(command), GUEST)
            self.assertIsNotNone(match, command)
            return int(match.group(1))

        client = timeout_for('"$gtk_panel"')
        drm = timeout_for('"$continuous_kms"')
        server = timeout_for('"$binary" --experimental-wayland-seat')
        self.assertLess(drm, client, "GTK4 USB recipient must outlive DRM")
        self.assertLess(client, server, "Smithay must own GTK client lifecycle")
        self.assertLessEqual(server, 60, "keep the test bounded")

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
