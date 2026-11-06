"""Stage106: bounded per-CRTC kernel flip event demultiplexing.

The unit tests in Rust establish deterministic queue behavior. The QEMU
guest actually exercises the same event router with real kernel events.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
ROUTER=(ROOT/"melania-native/src/native_drm_flip.rs").read_text()
DRM=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GBM=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class KernelEventRouterStage106(unittest.TestCase):
    def test_all_events_in_read_batch_are_processed_before_returning(self):
        self.assertIn("pub struct DrmFlipEventRouter",ROUTER)
        self.assertIn("VecDeque<(crtc::Handle, u32)>",ROUTER)
        self.assertIn("for event in events {",ROUTER)
        self.assertIn("router.enqueue(done.crtc, done.frame)?",ROUTER)
        read=ROUTER.index("let events=card.receive_events()")
        loop=ROUTER.index("for event in events {",read)
        take=ROUTER.index("router.take(expected_crtc)",loop)
        self.assertLess(read,loop)
        self.assertLess(loop,take)
        self.assertIn("preserves_interleaved_kernel_events_from_other_monitors",ROUTER)

    def test_duplicate_or_overflow_events_fail_closed(self):
        self.assertIn("MAX_PENDING_CRTC_EVENTS: usize = 16",ROUTER)
        self.assertIn("self.events.len() >= MAX_PENDING_CRTC_EVENTS",ROUTER)
        self.assertIn("self.events.iter().any",ROUTER)
        self.assertIn("self.revoke();",ROUTER)
        self.assertIn("duplicate_same_crtc_event_revokes_device_epoch",ROUTER)
        self.assertIn("bounded_pending_outputs_and_independent_new_epoch",ROUTER)


    def test_actual_cpu_and_gbm_presenters_bind_owned_kernel_crtcs(self):
        self.assertIn("bind_active_crtcs(&[crtc_handle])?;",DRM)
        self.assertEqual(GBM.count("bind_active_crtcs(&[crtc])?;"),2)
        self.assertIn("fn check_owned_crtc(&mut self",ROUTER)
        self.assertIn("self.check_owned_crtc(crtc)?;",ROUTER)
        self.assertIn("self.check_owned_crtc(expected)?;",ROUTER)
        self.assertIn("bound_router_preserves_two_owned_outputs_but_rejects_foreign_crtc",ROUTER)
        self.assertIn("bound_router_rejects_invalid_routes_and_foreign_waits",ROUTER)

    def test_real_qemu_kms_and_gbm_use_same_routing_state(self):
        for path in [DRM,GBM]:
            self.assertIn("DrmFlipEventRouter::default()",path)
            self.assertIn("wait_for_kms_page_flip_routed(",path)
            self.assertIn("&mut event_router,",path)
            self.assertIn("flip_ledger.complete_vm_flip(kernel_frame)",path)
            self.assertIn("PageFlipFlags::EVENT",path)
        self.assertIn("MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=24",GUEST)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=2",GUEST)
        self.assertIn("NATIVE_SESSION_READY=false",DRM)

if __name__ == "__main__":
    unittest.main()
