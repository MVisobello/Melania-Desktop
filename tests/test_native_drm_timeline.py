"""Source contracts for the real kernel DRM timeline capability probe."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
TIMELINE=(ROOT/"melania-native/src/native_drm_timeline.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class NativeKernelTimeline(unittest.TestCase):
    def test_real_drm_ioctls_and_cleanup(self):
        for marker in ("card.create_syncobj(false)", "card.fd_to_syncobj(fd,false)",
                       "syncobj_timeline_query(", "syncobj_timeline_signal(",
                       "syncobj_eventfd(", "syncobj_timeline_transfer(",
                       "syncobj_to_fd(binary.handle(),true)", "destroy_syncobj(handle)"):
            self.assertIn(marker,TIMELINE)
        self.assertIn("binary.close()?",TIMELINE)
        self.assertIn("consumer.close()?",TIMELINE)
        self.assertIn("producer.close()?",TIMELINE)
        self.assertIn("release.close()?",TIMELINE)

    def test_kernel_test_only_acquire_and_release_are_independent(self):
        self.assertLess(TIMELINE.index("consumer.ready(2)? || release.ready(7)?"),
                        TIMELINE.index("producer.signal(1)?"))
        self.assertLess(TIMELINE.index("producer.signal(1)?"),
                        TIMELINE.index("producer.signal(2)?"))
        self.assertLess(TIMELINE.index("consumer.export_sync_file(2)?"),
                        TIMELINE.index("release.signal(7)?"))
        self.assertIn("release.signal(7).is_ok()",TIMELINE)
        self.assertIn("BlockerState::Pending",TIMELINE)
        self.assertIn("AcquireFenceState::Ready",TIMELINE)

    def test_no_advertisement_or_false_success_on_missing_capabilities(self):
        self.assertIn("DriverCapability::TimelineSyncObj",TIMELINE)
        self.assertIn("TimelineProbe::Unsupported",TIMELINE)
        self.assertNotIn("DrmSyncobjState::new",TIMELINE)
        self.assertIn("probe_kernel_timelines(&fd)?",MAIN)
        self.assertIn("MELANIA_QEMU_REAL_DRM_SYNCOBJ_TIMELINE_EVENTFD_PASS=true",GUEST)
        self.assertIn("MELANIA_QEMU_DRM_SYNCOBJ_EVENTFD_UNSUPPORTED=true",GUEST)
        self.assertIn("MELANIA_QEMU_DRM_SYNCOBJ_TIMELINE_UNSUPPORTED=true",GUEST)
        self.assertIn("native_DRM_syncobj_capability_probe_missing",GUEST)

    def test_transaction_gate_checks_seat_and_actual_timeline_after_wake(self):
        gate=(ROOT/"melania-native/src/native_explicit_acquire.rs").read_text()
        self.assertIn("add_blocker(surface,gate.blocker())",gate)
        self.assertIn("point.eventfd()",gate)
        self.assertIn("Fence::is_signaled(&self.point)",gate)
        self.assertLess(gate.index("guard.accepts(self.token,seat_active)"),
                        gate.index("acquire_fence_state(fd)"))
        self.assertLess(gate.index("acquire_fence_state(fd)"),gate.index("match verify_point()"))
        self.assertIn("BlockerState::Cancelled",gate)
        self.assertIn("guard.revoke()",TIMELINE)
        self.assertIn("MELANIA_NATIVE_DRM_ACQUIRE_BLOCKER_VERIFIED=true",TIMELINE)
