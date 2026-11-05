"""Stage103: real DRM vblank event gate before acknowledging scanout.

These source checks are not hardware proof. QEMU guest exercises the actual
kernel page flip and must observe the logged event for both DRM and GBM.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
POLL=(ROOT/"melania-native/src/native_drm_flip.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
DRM=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GBM=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class VBlankGateContract(unittest.TestCase):
    def test_real_drm_fd_event_not_an_unverified_delay(self):
        self.assertIn("pub mod native_drm_flip;",LIB)
        self.assertIn("wait_for_kms_page_flip(",POLL)
        self.assertIn("PollFd::new(card, PollFlags::IN)",POLL)
        self.assertIn("poll(&mut fds,timeout_ms)",POLL)
        self.assertIn("card.receive_events()",POLL)
        self.assertIn("control::Event::PageFlip(done)",POLL)
        self.assertIn("router.enqueue(done.crtc, done.frame)?",POLL)
        self.assertIn("DRM page flip vblank event timed out",POLL)
        self.assertIn("PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL",POLL)

    def test_kernel_event_precedes_crtc_and_wayland_ack(self):
        self.assertIn("PageFlipFlags::EVENT",DRM)
        self.assertIn("PageFlipFlags::EVENT",GBM)
        self.assertNotIn("PageFlipFlags::empty()",DRM)
        self.assertNotIn("PageFlipFlags::empty()",GBM)
        start=DRM.index("if index>0 {\n                let kernel_frame=melania_native::native_drm_flip")
        event=DRM.index("wait_for_kms_page_flip_routed(",start)
        crtc=DRM.index("wait_for_fb(&card,crtc_handle,framebuffer)?;",event)
        receipt=DRM.index("stream.write_all(&ack)?",crtc)
        self.assertLess(event,crtc)
        self.assertLess(crtc,receipt)
        self.assertIn("verify_kernel_scanout(",DRM[crtc:receipt])
        self.assertIn("MELANIA_RUST_KMS_VBLANK_EVENT_ACK={}",DRM)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK={}",GBM)

    def test_qemu_must_prove_event_not_just_fb_id(self):
        self.assertIn("MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=2",GUEST)
        self.assertIn("MELANIA_RUST_KMS_VBLANK_EVENT_ACK=12",GUEST)
        self.assertIn("NATIVE_SESSION_READY=false",DRM)

if __name__=="__main__":
    unittest.main()
