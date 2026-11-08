"""Native dispatch must require original DRM encoder ownership for Wayland ACKs.

This checks the production Rust ioctl call path and stale-submission guard;
it does not claim physical VT/hotplug acceptance or replace real QEMU tests.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
DISPATCH = (ROOT / "melania-native/src/native_output_dispatch.rs").read_text()
KMS = (ROOT / "melania-native/src/native_kms_scanout.rs").read_text()


class DispatchOriginalEncoderFrameAck(unittest.TestCase):
    def test_original_encoder_is_part_of_the_unforgeable_dispatch_api(self):
        verify = DISPATCH.split("pub fn verify_and_present(", 1)[1].split(
            "fn presented(", 1
        )[0]
        self.assertIn("original_encoder: encoder::Handle", verify)
        self.assertIn("expected_size: (u16, u16)", verify)
        self.assertIn("expected_fb: framebuffer::Handle", verify)
        self.assertIn("self.inflight != Some(submission)", verify)
        self.assertIn("self.resources.phase() != RecoveryPhase::Active", verify)
        self.assertIn(
            "crate::native_kms_scanout::verify_original_encoder_frame(", verify
        )
        self.assertIn(
            "card, crtc, connector, original_encoder, expected_fb, expected_size,",
            verify,
        )
        self.assertNotIn("verify_kernel_scanout(", verify)
        self.assertLess(
            verify.index("self.inflight != Some(submission)"),
            verify.index("verify_original_encoder_frame("),
        )
        self.assertLess(
            verify.index("verify_original_encoder_frame("),
            verify.index("self.presented("),
        )
        self.assertIn("self.fail_closed();", verify)

    def test_exact_encoder_verifier_is_real_kernel_ioctls_not_boolean(self):
        verifier = KMS.split("pub fn verify_original_encoder_frame(", 1)[1].split(
            "pub fn verify_rollback_route(", 1
        )[0]
        self.assertIn("card.get_connector(connector,false)", verifier)
        self.assertIn("output.current_encoder()!=Some(original_encoder)", verifier)
        self.assertIn("card.get_encoder(original_encoder)", verifier)
        self.assertIn("routed.crtc()==Some(crtc)", verifier)
        self.assertIn("verify_presenter_ownership(", verifier)
        self.assertIn("original_route(card,crtc,connector,original_encoder)", verifier)

    def test_no_premature_wayland_success_or_native_login(self):
        block = DISPATCH.split("fn presented(", 1)[1].split("pub fn rejected(", 1)[0]
        for required in (
            "!kernel_presented",
            "!self.watchdog.acknowledged(",
            "!self.resources.acknowledge_frame(",
            "!self.redraw.presented(",
        ):
            self.assertIn(required, block)
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", DISPATCH)


if __name__ == "__main__":
    unittest.main()
