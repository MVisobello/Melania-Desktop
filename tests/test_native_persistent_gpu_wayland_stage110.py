"""Stage110: persistent GPU-backed real Wayland compositor lifecycle."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GPU=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
RUN=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
READER=(ROOT/"melania-native/src/native_persistent_frame_receive.rs").read_text()

class PersistentGpuWayland110(unittest.TestCase):
    def test_gpu_owner_has_no_idle_timeout_or_frame_budget_when_persistent(self):
        body=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        self.assertIn("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND",body)
        self.assertIn("MELANIA_NATIVE_PERSISTENT_KMS",body)
        self.assertIn("stream.set_read_timeout(if persistent {",body)
        self.assertIn("None\n    } else {\n        Some(Duration::from_secs(12))",body)
        self.assertIn("if !persistent && frames_seen>=TOTAL {break;}",body)
        # Idle is unlimited until a real header begins, but a hung/trickled
        # GPU image cannot retain the DRM/GBM CRTC beyond the deadline.
        self.assertIn("PersistentFrameRead::begin_with_idle_guard(",body)
        self.assertIn("MAX_PARTIAL_FRAME_TIME",body)
        self.assertIn("read_payload_guarded(",READER)
        self.assertIn("read_exact_before_deadline_guarded(",READER)
        self.assertIn("verify_original_encoder_frame_at_baseline(",body)
        self.assertIn("rollback.ownership_lost=true;",body)
        # The real CPU and GBM receivers now poll kernel scanout ownership
        # during unlimited idle; this is not an artificial timeout.
        self.assertIn("IDLE_OWNERSHIP_POLL",READER)
        self.assertIn("guard()?;",READER)

        self.assertIn("read.read_payload_guarded(&mut stream,&mut pixels,&mut verify_read_drm)?;",body)
        self.assertIn("stream.read_exact(&mut header)?;",body)
        self.assertIn("stream.read_exact(&mut pixels)?;",body)
        self.assertIn("stream.set_read_timeout(None)?;",READER)
        self.assertIn("Ok(0)=> {",READER)
        self.assertIn("stream.set_read_timeout(None)?;",READER)
        self.assertIn("ErrorKind::UnexpectedEof",READER)
        self.assertIn("saturating_duration_since(Instant::now())",READER)
        self.assertIn("frames_seen=frames_seen.checked_add(1)",body)
        self.assertIn("MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_ROLLBACK_PASS=true",body)
        self.assertIn("let restored=rollback.restore();",body)

    def test_persistent_gpu_mode_refuses_legacy_frame_budget_and_host_fallback(self):
        body=RUN[RUN.index("pub(crate) fn run_embedded("):
                 RUN.index("fn run_stream(",RUN.index("pub(crate) fn run_embedded("))]
        self.assertIn("guest_only()?;",body)
        self.assertIn('Some("YES_DISPOSABLE_QEMU")',body)
        self.assertIn("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND",body)
        self.assertIn("MELANIA_NATIVE_PERSISTENT_KMS",body)
        self.assertIn("std::env::var_os(\"MELANIA_QEMU_CONTINUOUS_FRAMES\").is_some()",body)
        self.assertIn("std::env::var_os(\"MELANIA_NATIVE_QEMU_FRAME_TARGET\").is_some()",body)
        self.assertIn('None=>{',body)
        self.assertIn("run_stream(stream,card)",body)
        self.assertIn("qemu_wayland_gpu_scene_stream(",body)
        self.assertIn("self.persistent_shutdown_requested",BRIDGE)

    def test_guest_client_environment_is_one_shell_command(self):
        """Do not detach WAYLAND_DISPLAY/48 frames from 'timeout' with comments."""
        stage=GUEST[GUEST.index("# Stage110: same REAL wl_shm"):GUEST.index("# Stage105:",GUEST.index("# Stage110: same REAL wl_shm"))]
        start=stage.index("if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM")
        end=stage.index('"$continuous_client" > "$persist_gpu_client_log" 2>&1; then',start)
        invocation=stage[start:end]
        lines=invocation.splitlines()
        # A comment after a line ending in a backslash terminates the Bash
        # logical command. This caused QEMU to run the default client instead
        # of the 48-frame client with its dedicated Wayland socket.
        for previous,current in zip(lines,lines[1:]):
            if previous.rstrip().endswith("\\"):
                self.assertFalse(current.lstrip().startswith("#"),
                                 "comment broke 48-frame client env chain")
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=48",invocation)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_IDLE_AFTER=24",invocation)
        self.assertIn('WAYLAND_DISPLAY="$persist_gpu_socket"',invocation)
        self.assertIn("timeout --signal=TERM --kill-after=3s 240s",invocation)

    def test_actual_qemu_waits_past_old_timeout_then_requires_kernel_restore(self):
        start=GUEST.index("# Stage110: same REAL wl_shm -> GLES -> GBM -> DRM pipeline")
        end=GUEST.index("MELANIA_QEMU_IN_PROCESS_PERSISTENT_GPU_WAYLAND_PASS=true",start)
        body=GUEST[start:end]
        for marker in (
            "MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU",
            "MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=48",
            "timeout --signal=TERM --kill-after=3s 240s",
            "MELANIA_QEMU_CONTINUOUS_CLIENT_IDLE_AFTER=24",
            "MELANIA_NATIVE_GPU_WAYLAND_CLIENT_RESUMED_AFTER_IDLE=true",
            "MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=47",
            "MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_STOPPED_FRAMES=",
            "MELANIA_QEMU_PERSISTENT_GPU_VERIFIED_FRAMES=",
            "MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=$index",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index",
            "MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=$((gpu_verified_frames - 1))",
            "sleep 15",
            'kill -0 "$persist_gpu_server_pid"',
            'kill -TERM "$persist_gpu_server_pid"',
            "MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_ROLLBACK_PASS=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "NATIVE_SESSION_READY=false",
        ):
            self.assertIn(marker,body)

if __name__=="__main__":
    unittest.main()
