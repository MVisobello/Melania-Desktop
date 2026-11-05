"""Stage112: real in-process KMS transport returns owned scene pixels after ACK.

This source contract is NOT hardware proof; QEMU still must verify real
DRM FB_ID, pageflip and the Wayland callback ordering.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
WORKER=(ROOT/"melania-native/src/continuous_kms_stream.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class NativeOwnedFramebufferAckStage112(unittest.TestCase):
    def test_worker_returns_same_owned_frame_allocation_with_receipt(self):
        self.assertIn("pub pixels: Vec<u8>",WORKER)
        self.assertIn("planned:job.planned,status,pixels:job.pixels",WORKER)
        self.assertIn("let status = transfer(&mut stream, &job);",WORKER)
        self.assertLess(WORKER.index("let status = transfer(&mut stream, &job);"),
                        WORKER.index("pixels:job.pixels"))
        self.assertIn("verified_ack(&ack, job.planned)",WORKER)
        self.assertIn("verified_ack_returns_same_scene_allocation_without_a_frame_clone",WORKER)
        self.assertIn("receipt.pixels.as_ptr(),original_ptr",WORKER)

    def test_bridge_never_copies_or_uses_unconfirmed_scene_for_damage(self):
        start=BRIDGE.index("fn submit_continuous_frame(")
        end=BRIDGE.index("fn poll_continuous_kms(",start)
        submit=BRIDGE[start:end]
        self.assertIn("self.dynamic_confirmed_frame.take()",submit)
        self.assertIn("composite_damaged_into(&mut previous)",submit)
        self.assertIn("worker.submit(ContinuousKmsJob{planned,pixels})",submit)
        self.assertNotIn("previous.clone()",submit)
        self.assertNotIn("pixels.clone()",submit)
        self.assertNotIn("dynamic_submitted_frame",BRIDGE)

    def test_receipt_is_promoted_only_after_kernel_verified_ack(self):
        start=BRIDGE.index("fn poll_continuous_kms(")
        end=BRIDGE.index("fn begin_generation_proof(",start)
        poll=BRIDGE[start:end]
        for marker in (
            "self.native_presentation_guard.accepts(ticket,self.lifecycle.seat_active())",
            "reply.planned!=planned",
            "if let Err(error)=reply.status",
            "if !self.continuous_redraw.presented(planned)",
            "self.dynamic_confirmed_frame=Some(reply.pixels)",
            "self.dynamic_scene.confirm_presented_damage(planned.generation)",
            "self.submit_continuous_frame()?",
        ):
            self.assertIn(marker,poll)
        self.assertLess(poll.index("if !self.continuous_redraw.presented(planned)"),
                        poll.index("self.dynamic_confirmed_frame=Some(reply.pixels)"))
        self.assertLess(poll.index("self.dynamic_confirmed_frame=Some(reply.pixels)"),
                        poll.index("self.submit_continuous_frame()?"))

if __name__=="__main__":
    unittest.main()
