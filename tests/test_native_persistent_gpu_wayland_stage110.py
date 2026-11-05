"""Stage110: persistent GPU-backed real Wayland compositor lifecycle."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GPU=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
RUN=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class PersistentGpuWayland110(unittest.TestCase):
    def test_gpu_owner_has_no_idle_timeout_or_frame_budget_when_persistent(self):
        body=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        self.assertIn("MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND",body)
        self.assertIn("MELANIA_NATIVE_PERSISTENT_KMS",body)
        self.assertIn("stream.set_read_timeout(if persistent {",body)
        self.assertIn("None\n    } else {\n        Some(Duration::from_secs(12))",body)
        self.assertIn("if !persistent && frames_seen>=TOTAL {break;}",body)
        self.assertIn("if stream.read(&mut header[..1])?==0 {break;}",body)
        self.assertIn("stream.read_exact(&mut header[1..])?;",body)
        self.assertIn("stream.read_exact(&mut pixels)?;",body)
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
