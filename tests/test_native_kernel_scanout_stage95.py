"""Stage95: only a live drm-rs CRTC and encoder read can release callbacks."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "melania-native/src/native_output_dispatch.rs").read_text()


class NativeKernelScanoutVerifierStage95(unittest.TestCase):
    def test_no_public_boolean_ack_bypass(self):
        self.assertNotIn("pub fn presented(", SOURCE)
        self.assertIn("fn presented(", SOURCE)
        self.assertIn("pub fn verify_and_present(", SOURCE)
        self.assertIn("Device as ControlDevice", SOURCE)

    def test_live_drm_crtc_framebuffer_and_encoder_route_both_required(self):
        block = SOURCE[SOURCE.index("pub fn verify_and_present("):SOURCE.index("fn presented(")]
        for marker in (
            "card.get_crtc(crtc)",
            "card.get_connector(connector, false)",
            "attached.current_encoder()",
            "card.get_encoder(encoder)",
            "observed.framebuffer() == Some(expected_fb)",
            "routed.crtc() == Some(crtc)",
            "Ok(self.presented(",
        ):
            self.assertIn(marker, block)
        self.assertLess(block.index("card.get_crtc("), block.index("self.presented("))
        self.assertLess(block.index("routed.crtc()"), block.index("self.presented("))

    def test_stale_ack_never_probes_or_cancels_new_frame(self):
        block = SOURCE[SOURCE.index("pub fn verify_and_present("):SOURCE.index("fn presented(")]
        self.assertIn("self.inflight != Some(submission)", block)
        self.assertLess(block.index("self.inflight != Some(submission)"), block.index("card.get_crtc(crtc)"))
        self.assertIn("return Ok(false);", block)

    def test_drm_read_errors_and_missing_route_fail_closed(self):
        block = SOURCE[SOURCE.index("pub fn verify_and_present("):SOURCE.index("fn presented(")]
        self.assertIn("let Some(encoder) = attached.current_encoder() else", block)
        self.assertIn("return Ok(false);", block)
        self.assertIn("self.fail_closed();", block)
        self.assertIn("Err(error)", block)

    def test_display_manager_stays_disabled(self):
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", SOURCE)


if __name__ == "__main__":
    unittest.main()
