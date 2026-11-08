"""Stage95: only live drm-rs CRTC and encoder reads may release callbacks."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "melania-native/src/native_output_dispatch.rs").read_text()
PROBE = (ROOT / "melania-native/src/native_kms_scanout.rs").read_text()


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
            "current.framebuffer() != Some(expected_fb)",
            "routed.crtc() == Some(crtc)",
        ):
            self.assertIn(marker, PROBE)
        self.assertIn("crate::native_kms_scanout::verify_kernel_scanout(", block)
        self.assertLess(block.index("verify_kernel_scanout("), block.index("self.presented("))
        self.assertIn("Ok(self.presented(", block)

    def test_ack_rejects_unplugged_monitor_and_changed_crtc_mode(self):
        block = PROBE[PROBE.index("pub fn verify_kernel_scanout("):
                      PROBE.index("pub fn verify_presenter_ownership(")]
        self.assertIn("attached.state() != connector::State::Connected", block)
        self.assertIn("let Some(initial_mode) = current.mode() else", block)
        self.assertIn("let initial_refresh = initial_mode.vrefresh()", block)
        self.assertIn("latest_crtc.mode() != Some(initial_mode)", block)
        self.assertNotIn("latest_crtc.mode().map(|mode| (mode.size(), mode.vrefresh()))", block)
        self.assertIn("latest_connector.current_encoder() != Some(encoder)", block)
        # The final encoder ioctl must reject a detached route before
        # the new final CRTC ownership check.
        self.assertIn("latest_encoder.crtc() != Some(crtc)", block)
        self.assertLess(block.index("latest_crtc.framebuffer()"),
                        block.index("latest_encoder.crtc()"))

    def test_final_kernel_crtc_read_brackets_the_last_encoder_route(self):
        block = PROBE[PROBE.index("pub fn verify_kernel_scanout("):
                      PROBE.index("pub fn verify_presenter_ownership(")]
        # A real get_crtc ioctl AFTER the final encoder ioctl prevents an
        # ACK for a framebuffer/mode/origin changed during the last route read.
        self.assertIn("let final_crtc = card.get_crtc(crtc)", block)
        self.assertIn("final_crtc.framebuffer() == Some(expected_fb)", block)
        self.assertIn("final_crtc.mode() == Some(initial_mode)", block)
        self.assertIn("final_crtc.position() == current.position()", block)
        self.assertLess(block.index("let latest_encoder = card.get_encoder(encoder)"),
                        block.index("let final_crtc = card.get_crtc(crtc)"))
        self.assertLess(block.index("let final_crtc = card.get_crtc(crtc)"),
                        block.index("Ok(final_crtc.framebuffer()"))
        self.assertNotIn("Ok(latest_encoder.crtc() == Some(crtc))", block)

    def test_presenter_rechecks_kernel_crtc_after_last_encoder_read(self):
        block = PROBE[PROBE.index("pub fn verify_presenter_ownership("):
                      PROBE.index("pub fn verify_original_encoder_frame(")]
        # This path runs before the next CPU/GBM KMS frame is submitted.
        # Route-only ACKs must not survive a final modeset or FB takeover.
        self.assertIn("if latest_routing.crtc()!=Some(crtc)", block)
        self.assertIn("let final_crtc=card.get_crtc(crtc)", block)
        self.assertIn("final_crtc.framebuffer()==Some(expected_fb)", block)
        self.assertIn("final_crtc.mode()==Some(initial_mode)", block)
        self.assertIn("final_crtc.position()==current.position()", block)
        self.assertLess(block.index("let latest_routing=card.get_encoder(encoder)"),
                        block.index("let final_crtc=card.get_crtc(crtc)"))
        self.assertLess(block.index("if latest_routing.crtc()!=Some(crtc)"),
                        block.index("let final_crtc=card.get_crtc(crtc)"))
        self.assertIn('DRM owner final get_crtc failed:', block)

    def test_presenter_owner_rejects_same_resolution_mode_timing_switch(self):
        block = PROBE[PROBE.index("pub fn verify_presenter_ownership("):
                      PROBE.index("pub fn owner_snapshot_matches(")]
        self.assertIn("let Some(initial_mode)=current.mode() else", block)
        self.assertIn("initial_mode.size()!=expected_size", block)
        self.assertIn("latest_crtc.mode()!=Some(initial_mode)", block)
        self.assertNotIn("latest_crtc.mode().map(|mode|mode.size())", block)

    def test_stale_ack_never_probes_or_cancels_new_frame(self):
        block = SOURCE[SOURCE.index("pub fn verify_and_present("):SOURCE.index("fn presented(")]
        self.assertIn("self.inflight != Some(submission)", block)
        self.assertLess(block.index("self.inflight != Some(submission)"),
                        block.index("verify_kernel_scanout("))
        self.assertIn("return Ok(false);", block)

    def test_drm_read_errors_and_missing_route_fail_closed(self):
        block = SOURCE[SOURCE.index("pub fn verify_and_present("):SOURCE.index("fn presented(")]
        self.assertIn("let Some(encoder) = attached.current_encoder() else", PROBE)
        self.assertIn("return Ok(false);", PROBE)
        self.assertIn("self.fail_closed();", block)
        self.assertIn("Err(error)", block)

    def test_display_manager_stays_disabled(self):
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", SOURCE)


if __name__ == "__main__":
    unittest.main()
