"""Stage93: QEMU receiver must restore connector/CRTC as well as FB_ID.

This is a structural regression test. The real proof is the QEMU workflow,
which checks kernel state inside the disposable VM after the visible scanout.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RECEIVER = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()


class NativeConnectorRestoreStage93(unittest.TestCase):
    def test_restore_verifies_original_crtc_and_connector_encoder_route(self):
        begin = RECEIVER.index("impl Restore<'_>")
        end = RECEIVER.index("impl Drop for Restore", begin)
        block = RECEIVER[begin:end]
        self.assertIn("self.card.set_crtc(old.handle()", block)
        self.assertIn("self.card.get_crtc(old.handle())?", block)
        self.assertIn("self.card.get_connector(self.connector,false)?", block)
        self.assertIn(".current_encoder()", block)
        self.assertIn(".and_then(|encoder|self.card.get_encoder(encoder).ok())", block)
        self.assertIn(".and_then(|encoder|encoder.crtc())==Some(old.handle())", block)
        self.assertIn("&& connector_attached", block)
        self.assertIn("MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED={connector_attached}", block)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED={}", block)

    def test_failed_modeset_still_enters_rollback_guard(self):
        block = RECEIVER[RECEIVER.index("if index==0 {"):]
        self.assertLess(block.index("rollback.submitted=true;"),
                        block.index("card.set_crtc(crtc_handle,Some(framebuffer)"))
        self.assertIn("impl Drop for Restore<'_>", RECEIVER)
        self.assertIn("if self.submitted && !self.restored", RECEIVER)

    def test_every_continuous_guest_scenario_checks_connector_restoration(self):
        for log in (
            "gtk_kms_drm_log", "gtk2_drm_log", "continuous_kms_log",
            "two_kms_log", "reopen_kms_log", "dynamic_kms_log",
            "managed_kms_log", "layer_kms_log",
        ):
            self.assertIn(
                "MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' \"$" + log + "\"",
                GUEST,
            )

    def test_hardware_scope_is_still_qemu_only(self):
        self.assertIn("fn guest_only()", RECEIVER)
        self.assertIn('YES_DISPOSABLE_QEMU', RECEIVER)
        self.assertIn('/dev/tty1', RECEIVER)
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false', RECEIVER)


if __name__ == "__main__":
    unittest.main()
