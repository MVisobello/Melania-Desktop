"""Check both REAL Rust DRM presenters share one incomplete-frame deadline.

Supplemental source wiring checks; Rust UnixStream tests prove actual blocking
read/EOF/trickle behavior. Neither test is hardware DRM or VT acceptance.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
CPU=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GPU=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
READER=(ROOT/"melania-native/src/native_persistent_frame_receive.rs").read_text()

class PersistentPartialFrameDeadline(unittest.TestCase):
    def test_real_cpu_and_gpu_share_bounded_header_and_payload(self):
        self.assertIn("pub mod native_persistent_frame_receive;",LIB)
        for presenter in (CPU,GPU):
            stream=presenter[presenter.index("let pending_read=if persistent {"):]
            self.assertIn("PersistentFrameRead::begin_with_idle_guard(",stream)
            self.assertIn("MAX_PARTIAL_FRAME_TIME",stream)
            self.assertIn("verify_original_encoder_frame_at_baseline(",stream)
            self.assertIn("ownership_lost=true;",stream)
            self.assertIn("read.read_payload_guarded(&mut stream,&mut pixels,&mut verify_read_drm)?;",stream)
            self.assertLess(stream.index("read.read_payload_guarded(&mut stream,&mut pixels,&mut verify_read_drm)?;"),
                            stream.index("stream.write_all(&ack)?;"))
            # Finite 24-frame KMS proof keeps its previous real read path.
            if presenter is CPU:
                self.assertIn("stream.read_exact(&mut header).map_err(",presenter)
                self.assertIn("stream.read_exact(&mut pixels).map_err(",presenter)
            else:
                self.assertIn("stream.read_exact(&mut header)?;",presenter)
                self.assertIn("stream.read_exact(&mut pixels)?;",presenter)
        self.assertIn("let restored=rollback.restore();",CPU)
        self.assertIn("let restored=rollback.restore();",GPU)

    def test_idle_time_unlimited_but_partial_frame_has_one_deadline(self):
        self.assertIn("stream.set_read_timeout(None)?;",READER)
        self.assertIn("deadline:Instant",READER)
        # The payload and fragmented header now enforce the same real DRM
        # owner guard; the original fixed partial-frame deadline is retained.
        self.assertIn("read_payload_guarded(",READER)
        self.assertIn("read_exact_before_deadline_guarded(",READER)
        # The real CPU and GBM receivers now poll kernel scanout ownership
        # during unlimited idle; this is not an artificial timeout.
        self.assertIn("IDLE_OWNERSHIP_POLL",READER)
        self.assertIn("guard()?;",READER)

        self.assertIn("saturating_duration_since(Instant::now())",READER)
        self.assertIn("stream.set_read_timeout(Some(remaining.min(IDLE_OWNERSHIP_POLL)))?;",READER)
        self.assertIn("ErrorKind::UnexpectedEof",READER)
        self.assertIn("trickled_payload_cannot_extend_original_header_deadline",READER)
        self.assertIn("real_fragmented_header_and_pixels_complete_before_one_deadline",READER)
        self.assertIn("partial_payload_timeout_never_looks_like_clean_shutdown",READER)

if __name__=="__main__":
    unittest.main()
