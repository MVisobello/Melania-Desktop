"""Stage44: independent continuous KMS wire-format safeguards.

These are source-contract checks; the actual protocol behavior is exercised
by Rust socket-pair unit tests. No real DRM/KMS scanout is claimed.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
WIRE = (ROOT / "melania-native/src/continuous_kms_stream.rs").read_text()
LIB = (ROOT / "melania-native/src/lib.rs").read_text()
OLD = (ROOT / "melania-native/src/async_kms_stream.rs").read_text()
BRIDGE = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
CI = (ROOT / ".github/workflows/linux-verify.yml").read_text()

class ContinuousKmsWireContract(unittest.TestCase):
    def test_parallel_not_replacement_of_six_frame_qemu_path(self):
        self.assertIn("pub mod continuous_kms_stream;", LIB)
        self.assertIn("AsyncKmsWorker", BRIDGE)
        self.assertIn("!(1..=6).contains(&job.ordinal)", OLD)
        self.assertNotIn("ContinuousKmsWorker", BRIDGE)
        self.assertIn("cargo test -p melania-native --all-targets", CI)

    def test_wire_checks_identity_and_does_not_touch_gpu(self):
        for token in (
            'b"MLF1"', 'b"MLA1"', "planned.ticket.epoch",
            "planned.ticket.sequence", "planned.generation",
            "stream.read_exact(&mut ack)", "verified_ack(&ack, job.planned)",
            "mpsc::sync_channel::<ContinuousKmsJob>(1)",
            "self.sender.try_send(job)", "self.receiver.try_recv()",
            "stale, reordered, or mismatched continuous KMS ACK",
        ):
            self.assertIn(token, WIRE)
        for forbidden in ("DrmDevice", "libseat", "NATIVE_SESSION_READY=true"):
            self.assertNotIn(forbidden, WIRE)

    def test_rust_socket_tests_cover_more_than_six_and_failures(self):
        for name in (
            "ten_consecutive_generations_without_six_frame_ceiling",
            "polling_does_not_block_while_peer_delays_ack",
            "stale_epoch_or_generation_cannot_acknowledge_frame",
            "failed_ack_and_invalid_payload_never_report_success",
        ):
            self.assertIn("fn " + name + "()", WIRE)

if __name__ == "__main__":
    unittest.main()
