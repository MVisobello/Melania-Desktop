"""Guard rail: real Smithay libseat + DRM udev changes share one QEMU loop.

Static integration checks supplement Rust tests; do not claim hotplug on
physical hardware, nor substitute for running a real QEMU guest.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
WATCH=(ROOT/"melania-native/src/native_gpu_watch.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class RealUdevKmsRevocation(unittest.TestCase):
    def test_actual_udev_event_source_is_wired_to_same_wayland_loop(self):
        self.assertIn("UdevBackend::new(&seat_name)?",BRIDGE)
        self.assertIn("events.handle().insert_source(udev",BRIDGE)
        self.assertIn("let reason=state.gpu_watch.observe(&change);",BRIDGE)
        self.assertIn("state.lifecycle.on_udev(event);",BRIDGE)
        self.assertIn("gpu_watch:NativeGpuWatch",BRIDGE)
        self.assertIn("pub mod native_gpu_watch;",LIB)

    def test_kms_must_revoke_before_dropping_worker_and_clients(self):
        start=BRIDGE.index("if let Some(reason)=reason {")
        end=BRIDGE.index("MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED",start)
        handler=BRIDGE[start:end]
        self.assertLess(handler.index("native_presentation_guard.revoke()"),
                        handler.index("revoke_wayland_input_for_lost_seat(state);"))
        self.assertLess(handler.index("revoke_wayland_input_for_lost_seat(state);"),
                        handler.index("continuous_worker.take()"))
        for term in ("continuous_inflight.take()","continuous_presentation_token.take()",
                     "continuous_redraw.set_seat_active(false)",
                     "udev_input_context.suspend();"):
            self.assertIn(term,handler)
        # The same real Rust cleanup must remove retained scene pixels and
        # interactive move/resize grabs before the KMS worker is dropped.
        revoker=BRIDGE[BRIDGE.index("fn revoke_wayland_input_for_lost_seat("):
                       BRIDGE.index("/// Keep the original libseat-owned descriptor")]
        for term in ("dynamic_confirmed_frame=None","window_interaction.cancel()",
                     "state.grabbed_popup_roles.clear();",
                     "set_primary_focus(&state.display_handle,&state.seat,None);"):
            self.assertIn(term,revoker)
        self.assertIn("native_presentation_guard.permanently_revoked()",BRIDGE)
        self.assertIn("libseat paused or DRM hotplug",BRIDGE)


    def test_no_implicit_gpu_rearming_or_native_login(self):
        self.assertIn("pub fn revoked(&self)->bool",WATCH)
        self.assertNotIn("native_login_authorized() -> true",WATCH)
        self.assertNotIn("libc::ioctl",WATCH)
        self.assertNotIn("unsafe {",WATCH)
        self.assertIn("NATIVE_SESSION_READY=false",BRIDGE)
        self.assertIn("BaselineConflict",WATCH)
        self.assertIn("Replaced",WATCH)
        self.assertIn("Removed",WATCH)

if __name__=="__main__":
    unittest.main()
