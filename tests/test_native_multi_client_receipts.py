"""Stage 50: independently held Wayland buffer receipts must survive overlap.

These checks supplement Rust ledger unit tests; actual GPU presentation is
tested separately in the existing disposable QEMU pipeline.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
LEDGER=(ROOT/"melania-native/src/client_commit_ledger.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class MultiSurfaceKmsReceipts(unittest.TestCase):
    def test_bounded_receipts_live_in_rust_library(self):
        self.assertIn("pub mod client_commit_ledger;",LIB)
        self.assertIn("pub const MAX_PENDING_CLIENTS: usize = 2;",LEDGER)
        for fn in ("two_clients_survive_overlapping_commits",
                   "duplicate_surface_is_bounded_and_old_buffer_returned",
                   "other_surface_updates_do_not_starve_old_client"):
            self.assertIn("fn "+fn+"()",LEDGER)
        self.assertIn("fn acknowledged(",LEDGER)
        self.assertIn("fn take_for_presentation(",LEDGER)

    def test_smithay_uses_client_specific_receipt_not_one_global_latest(self):
        self.assertIn("ClientCommitLedger<WlSurface,wl_buffer::WlBuffer>",BRIDGE)
        self.assertIn("self.continuous_client_ledger.stage(surface.clone(),buffer)",BRIDGE)
        self.assertIn("self.continuous_client_ledger.take_for_presentation()",BRIDGE)
        self.assertIn("self.continuous_client_ledger.acknowledged(clients)",BRIDGE)
        self.assertNotIn("continuous_latest_client:Option",BRIDGE)
        self.assertNotIn("self.continuous_latest_client.take()",BRIDGE)

    def test_frame_callback_never_precedes_drm_proof_and_buffer_released(self):
        start=BRIDGE.index("fn poll_continuous_kms(&mut self)")
        end=BRIDGE.index("fn begin_generation_proof",start)
        section=BRIDGE[start:end]
        self.assertLess(section.index("self.continuous_redraw.presented(planned)"),
                        section.index("self.continuous_client_ledger.acknowledged(clients)"))
        ledger=section.index("self.continuous_client_ledger.acknowledged(clients)")
        # Stage49 also has an independent compositor-only close callback,
        # already verified after the KMS ACK. Inspect the per-surface callback
        # that belongs to the NEW ledger, not the earlier close callback.
        frame=section.index("complete_presented_frame(",ledger)
        release=section.index("completed.resource.release();",ledger)
        self.assertLess(ledger,frame)
        self.assertLess(frame,release)
        self.assertIn("completed.notify_frame && is_live",section)
        self.assertIn("self.known_window_surfaces.get(slot)==Some(&completed.key)",section)

if __name__=="__main__":
    unittest.main()
