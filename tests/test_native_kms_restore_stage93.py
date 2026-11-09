"""Stage93: actual KMS rollback must restore original kernel encoder route.

Structural contract: the CPU and GPU Rust Restore guards use the same
fail-closed DRM kernel ioctl helper, including post-modeset verification.
The complete QEMU guest independently proves real framebuffer/route restore.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RECEIVER = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GPU = (ROOT / "melania-native/src/native_gbm_scanout.rs").read_text()
SCANOUT = (ROOT / "melania-native/src/native_kms_scanout.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()


def restore_block(source):
    begin = source.index("impl Restore<'_>")
    end = source.index("impl Drop for Restore", begin)
    return source[begin:end]


def kernel_rollback_helper():
    begin = SCANOUT.index("pub fn verify_rollback_route(")
    end = SCANOUT.index("/// Pure snapshot predicate", begin)
    return SCANOUT[begin:end]


class NativeConnectorRestoreStage93(unittest.TestCase):
    def test_restore_verifies_original_crtc_and_connector_encoder_route(self):
        block = restore_block(RECEIVER)
        helper = kernel_rollback_helper()
        self.assertIn("self.card.set_crtc(old.handle()", block)
        self.assertEqual(block.count("verify_rollback_route("), 2)
        # The original encoder is an *identity*, not just any encoder that
        # happens to route to the same CRTC after another GPU owner takes over.
        self.assertIn("original_encoder", block)
        self.assertIn("original_encoder=card.get_connector(route.connector,false)?", RECEIVER)
        self.assertIn("output.current_encoder()!=Some(expected_encoder)", helper)
        self.assertIn("encoder.crtc()!=Some(original.handle())", helper)
        # Full original DRM mode, position and original framebuffer must
        # survive the final KMS restore; owned buffers are NOT accepted after.
        self.assertIn("current.mode()!=original.mode()", helper)
        self.assertIn("current.position()!=original.position()", helper)
        self.assertIn("original.framebuffer()!=Some(fb)", helper)
        self.assertIn("self.original_encoder,&[]", block)
        self.assertIn("self.restored=connector_attached;", block)
        self.assertIn("MELANIA_RUST_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true", block)
        self.assertIn("MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED={connector_attached}", block)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED={}", block)

    def test_cpu_and_gbm_restore_both_fail_closed_on_verification_io_errors(self):
        helper = kernel_rollback_helper()
        # The new central helper still reads the REAL three DRM kernel
        # objects twice and propagates all ioctl errors, rather than
        # replacing a previous safety check with a permissive bool.
        for call in (
            "card.get_crtc(original.handle())",
            "card.get_connector(connector,false)",
            "card.get_encoder(expected_encoder)",
        ):
            self.assertIn(call, helper)
        self.assertIn(".map_err(|e|format!", helper)
        self.assertIn("let first=snapshot(", helper)
        self.assertIn("let second=snapshot(", helper)
        self.assertIn("if first!=second {return Ok(false);}", helper)
        self.assertIn("if first.is_none() {return Ok(false);}", helper)
        self.assertIn("output.state()!=connector::State::Connected", helper)
        self.assertIn("!owned_fbs.contains(&fb)", helper)

        for source in (RECEIVER, GPU):
            restore = restore_block(source)
            self.assertIn("self.card.set_crtc(old.handle()", restore)
            self.assertEqual(restore.count("verify_rollback_route("), 2)
            before = restore.index("verify_rollback_route(")
            modeset = restore.index("self.card.set_crtc(old.handle()")
            after = restore.rindex("verify_rollback_route(")
            self.assertLess(before, modeset)
            self.assertLess(modeset, after)
            # BOTH pre- and post-modeset ioctl failures must permanently
            # revoke Drop retry, with no "ok()" or swallowed errors.
            self.assertGreaterEqual(
                restore.count("self.ownership_lost=true;"), 4
            )
            self.assertIn(
                "self.original_encoder,&self.owned_fbs,", restore
            )
            self.assertIn(
                "self.original_encoder,&[],", restore
            )
            self.assertNotIn(".get_encoder(encoder).ok()", restore)
            failed = restore[restore.index("if !self.restored {"):]
            self.assertLess(
                failed.index("self.ownership_lost=true;"),
                failed.index("return Err("),
            )
            drop = source[source.index("impl Drop for Restore", source.index("impl Restore<'_>")):]
            self.assertIn(
                "if self.submitted && !self.restored && !self.ownership_lost",
                drop,
            )

    def test_rollback_final_kernel_crtc_follows_last_encoder_read(self):
        helper = kernel_rollback_helper()
        # Real drm-rs ioctls in the shared CPU/GBM pre/post rollback route,
        # never a simulated framebuffer success flag.
        self.assertIn('let final_crtc=card.get_crtc(original.handle())', helper)
        self.assertIn('DRM rollback final get_crtc failed:', helper)
        self.assertIn('final_crtc.framebuffer()==first', helper)
        self.assertIn('final_crtc.mode()==original.mode()', helper)
        self.assertIn('final_crtc.position()==original.position()', helper)
        self.assertLess(helper.index('let second=snapshot('),
                        helper.index('let final_crtc=card.get_crtc('))
        self.assertLess(helper.index('if first!=second {return Ok(false);}'),
                        helper.index('let final_crtc=card.get_crtc('))
        self.assertNotIn('Ok(first==second)', helper)

    def test_real_qemu_proves_both_gpu_and_cpu_original_encoder_restore(self):
        self.assertIn(
            "'MELANIA_NATIVE_GBM_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true'",
            GUEST,
        )
        self.assertIn(
            "'MELANIA_RUST_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true'",
            GUEST,
        )
        self.assertIn(
            "MELANIA_NATIVE_GBM_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true",
            GPU,
        )

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
        self.assertIn("YES_DISPOSABLE_QEMU", RECEIVER)
        self.assertIn("/dev/tty1", RECEIVER)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false", RECEIVER)


if __name__ == "__main__":
    unittest.main()
