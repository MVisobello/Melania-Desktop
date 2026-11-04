"""Fail-closed compositor VT pause must interrupt real KMS frame transport.

This static contract supplements existing Rust socket-pair shutdown tests.
It does not claim a physical VT switch or production DRM re-acquisition.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
WORKER=(ROOT/"melania-native/src/continuous_kms_stream.rs").read_text()
GUARD=(ROOT/"melania-native/src/native_presentation_guard.rs").read_text()

class NativeVtTransportShutdown(unittest.TestCase):
    def test_pause_stops_kms_transport_before_input_suspend(self):
        start=BRIDGE.index("SessionEvent::PauseSession=>{")
        end=BRIDGE.index("SessionEvent::ActivateSession=>{",start)
        pause=BRIDGE[start:end]
        for expected in (
            "state.native_presentation_guard.revoke();",
            "state.continuous_worker.take();",
            "state.continuous_inflight.take();",
            "state.continuous_presentation_token.take();",
            "state.continuous_pending=None;",
            "context.suspend();",
        ):
            self.assertIn(expected,pause)
        self.assertLess(pause.index("state.native_presentation_guard.revoke();"),
                        pause.index("state.continuous_worker.take();"))
        self.assertLess(pause.index("state.continuous_worker.take();"),
                        pause.index("context.suspend();"))

    def test_failed_libinput_resume_cannot_reuse_old_dma_frame(self):
        start=BRIDGE.index("if context.resume().is_err()")
        end=BRIDGE.index("return;",start)
        recovery=BRIDGE[start:end]
        self.assertIn("state.native_presentation_guard.revoke();",recovery)
        self.assertIn("state.continuous_worker.take();",recovery)
        self.assertIn("state.continuous_inflight.take();",recovery)
        self.assertIn("state.continuous_presentation_token.take();",recovery)
        self.assertIn("state.lifecycle.on_session(SessionEvent::PauseSession);",recovery)
        self.assertIn("state.continuous_redraw.set_seat_active(false);",recovery)

    def test_stopped_worker_has_no_live_socket_or_stale_callback(self):
        self.assertIn("impl Drop for ContinuousKmsWorker",WORKER)
        self.assertIn("self.shutdown.shutdown(Shutdown::Both)",WORKER)
        self.assertIn("worker.join()",WORKER)
        self.assertIn("self.sender.take();",WORKER)
        self.assertIn("Err(TrySendError::Disconnected(job))",WORKER)
        self.assertIn("pub fn permanently_revoked(&self)->bool",GUARD)
        self.assertIn("if state.native_presentation_guard.permanently_revoked()",BRIDGE)
        self.assertIn("if integrated_kms && run_result.is_err()",BRIDGE)
        start=BRIDGE.index("if integrated_kms && run_result.is_err()")
        cleanup=BRIDGE[start:]
        self.assertLess(cleanup.index("state.continuous_worker.take()"),
                        cleanup.index("join.join()"))
        self.assertLess(cleanup.index("join.join()"),
                        cleanup.index("owner.release()"))
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",BRIDGE)

if __name__ == "__main__":
    unittest.main()
