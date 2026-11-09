"""Stage105: opt-in persistent real-QEMU Wayland/DRM lifecycle contract.

Source assertions are safeguards, not DRM hardware proof. The actual QEMU
workflow must render 24 real Wayland frames, remain up and verify rollback.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
CARGO=(ROOT/"melania-native/Cargo.toml").read_text()
READER=(ROOT/"melania-native/src/native_persistent_frame_receive.rs").read_text()

class PersistentKmsStage105(unittest.TestCase):
    def test_qemu_seat_and_explicit_one_process_opt_in_remain_mandatory(self):
        for marker in (
            "MELANIA_NATIVE_PERSISTENT_KMS",
            "YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_IN_PROCESS_KMS",
            "MELANIA_QEMU_CONTINUOUS_KMS",
            "persistent KMS requires in-process QEMU",
        ):
            self.assertIn(marker,BRIDGE+RECEIVER)
        self.assertIn("guest_only()?;",RECEIVER)
        self.assertIn("signal-hook",CARGO)
        self.assertIn("NATIVE_SESSION_READY=false",BRIDGE)
        self.assertIn("NATIVE_SESSION_READY=false",RECEIVER)

    def test_receiver_reads_until_complete_frame_boundary_then_restores(self):
        self.assertIn("let total=if persistent {None} else",RECEIVER)
        self.assertIn("if total.is_some_and(|limit|frames_seen>=limit)",RECEIVER)
        # Frame-boundary EOF is allowed but an incomplete frame must abort
        # with no Wayland KMS ACK, even if the sender trickles bytes.
        self.assertIn("PersistentFrameRead::begin_with_idle_guard(",RECEIVER)
        self.assertIn("let Some(read)=",RECEIVER)
        self.assertIn("MAX_PARTIAL_FRAME_TIME",RECEIVER)
        self.assertIn("read_payload_guarded(",READER)
        self.assertIn("read_exact_before_deadline_guarded(",READER)
        self.assertIn("verify_original_encoder_frame(",RECEIVER)
        self.assertIn("rollback.ownership_lost=true;",RECEIVER)
        # The real CPU and GBM receivers now poll kernel scanout ownership
        # during unlimited idle; this is not an artificial timeout.
        self.assertIn("IDLE_OWNERSHIP_POLL",READER)
        self.assertIn("guard()?;",READER)

        self.assertIn("read.read_payload_guarded(&mut stream,&mut pixels,&mut verify_read_drm)?;",RECEIVER)
        self.assertIn("stream.read_exact(&mut header)?;",RECEIVER)
        self.assertIn("stream.read_exact(&mut pixels)?;",RECEIVER)
        self.assertIn("Ok(0)=> {",READER)
        self.assertIn("stream.set_read_timeout(None)?;",READER)
        self.assertIn("read_exact_before_deadline_guarded(stream,&mut header[1..],&mut guard)?;",READER)
        self.assertIn("self.read_exact_before_deadline_guarded(stream,pixels,&mut guard)?;",READER)
        self.assertIn("ErrorKind::UnexpectedEof",READER)
        self.assertIn("saturating_duration_since(Instant::now())",READER)
        self.assertNotIn("persistent DRM closed before first frame",RECEIVER)
        self.assertIn("let restored=rollback.restore();",RECEIVER)
        self.assertIn("MELANIA_RUST_PERSISTENT_KMS_STOPPED_FRAMES={frames_seen}",RECEIVER)
        self.assertIn("MELANIA_RUST_CONTINUOUS_FRAMES_PASS={total}",RECEIVER)

    def test_wayland_runs_until_signal_and_revokes_before_device_release(self):
        self.assertIn("signal_hook::flag::register(signal,shutdown_flag.clone())?",BRIDGE)
        self.assertIn("shutdown_flag.load(Ordering::Relaxed)",BRIDGE)
        self.assertIn("state.persistent_shutdown_requested=true;",BRIDGE)
        self.assertIn("if state.continuous_inflight.is_some()",BRIDGE)
        section=BRIDGE[BRIDGE.index("if persistent_kms && shutdown_flag.load(Ordering::Relaxed)"):]
        self.assertLess(section.index("state.native_presentation_guard.revoke()"),
                        section.index("state.continuous_worker.take()"))
        self.assertLess(section.index("state.continuous_worker.take()"),
                        section.index("join.join()"))
        self.assertLess(section.index("join.join()"),
                        section.index("owner.release()"))
        self.assertIn("MELANIA_NATIVE_PERSISTENT_KMS_SHUTDOWN_OK=true",section)

    def test_real_virtual_gpu_guest_checks_process_lifetime_and_rollback(self):
        start=GUEST.index("# Stage105: an actual ONE-PROCESS QEMU DRM/Wayland compositor")
        end=GUEST.index("MELANIA_QEMU_IN_PROCESS_PERSISTENT_KMS_SIGTERM_ROLLBACK_PASS=true",start)
        scenario=GUEST[start:end]
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24",scenario)
        self.assertIn("MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",scenario)
        self.assertIn("kill -0 \"$persistent_server_pid\"",scenario)
        self.assertIn("kill -TERM \"$persistent_server_pid\"",scenario)
        self.assertIn("MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true",scenario)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED=true",scenario)
        self.assertIn("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",scenario)

if __name__=="__main__":
    unittest.main()
