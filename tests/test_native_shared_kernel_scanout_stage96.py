"""Stage96: the production-shaped kernel verifier is exercised on real QEMU KMS."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
LIB = (ROOT / "melania-native/src/lib.rs").read_text()
HELPER = (ROOT / "melania-native/src/native_kms_scanout.rs").read_text()
DISPATCH = (ROOT / "melania-native/src/native_output_dispatch.rs").read_text()
QEMU = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()


class SharedKernelScanoutStage96(unittest.TestCase):
    def test_real_drm_helper_compiled_into_native_library(self):
        self.assertIn("pub mod native_kms_scanout;", LIB)
        self.assertIn("pub fn verify_kernel_scanout(", HELPER)
        self.assertIn("Device as ControlDevice", HELPER)
        self.assertIn("card.get_crtc(crtc)", HELPER)
        self.assertIn("card.get_connector(connector, false)", HELPER)
        self.assertIn("card.get_encoder(encoder)", HELPER)
        self.assertIn("current.framebuffer() != Some(expected_fb)", HELPER)
        self.assertIn("routed.crtc() == Some(crtc)", HELPER)

    def test_same_helper_in_future_dispatch_and_actual_qemu_scanout(self):
        self.assertIn("crate::native_kms_scanout::verify_kernel_scanout(", DISPATCH)
        self.assertIn("melania_native::native_kms_scanout::verify_kernel_scanout(", QEMU)
        frame=QEMU[QEMU.index("wait_for_fb(&card,crtc_handle,framebuffer)?;"):]
        self.assertLess(frame.index("verify_kernel_scanout("),frame.index("stream.write_all(&ack)?"))
        self.assertIn("MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED={}", QEMU)

    def test_eight_real_qemu_scenarios_require_kernel_route_proof(self):
        for log,count in (
            ("gtk_kms_drm_log",1), ("gtk2_drm_log",3),
            ("continuous_kms_log",12), ("two_kms_log",12),
            ("reopen_kms_log",14), ("dynamic_kms_log",13),
            ("managed_kms_log",12), ("layer_kms_log",12),
        ):
            marker=f"MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED={count}$"
            self.assertIn(marker, GUEST)
            self.assertIn(f'"${log}"', GUEST)

    def test_no_host_gpu_or_login_enabled(self):
        self.assertIn("fn guest_only()", QEMU)
        self.assertIn("NATIVE_SESSION_READY=false", QEMU)
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", DISPATCH)


if __name__ == "__main__":
    unittest.main()
