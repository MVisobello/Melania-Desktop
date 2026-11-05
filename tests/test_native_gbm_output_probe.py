"""Native GPU rendering at real output resolutions: no DRM modeset or login.

The Rust unit tests cover the memory budget. These Python checks protect the
CLI/device safety contract; only a dedicated-console GPU test exercises EGL.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RENDERER = (ROOT / "melania-native/src/renderer.rs").read_text()
NATIVE = (ROOT / "melania-native/src/main.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
HOST = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()

class NativeGbmOutputProbe(unittest.TestCase):
    def test_real_output_sized_gpu_path_reuses_libseat_device(self):
        self.assertIn('fn experimental_drm(render_offscreen: bool, output_sized: bool)', NATIVE)
        self.assertIn('assert_dedicated_console();', NATIVE)
        self.assertIn('session.open(', NATIVE)
        self.assertIn('inspect_connectors(device.device_fd())', NATIVE)
        self.assertIn('recommended_mode_for_budget(', NATIVE)
        self.assertIn('smoke_render_gbm_output_buffers(', NATIVE)
        # The kernel topology, not one EDID-derived mode, drives GPU probes.
        self.assertIn('inspect_stable_active_outputs(', NATIVE)
        self.assertIn('plan_active_output_probes(&topology)', NATIVE)
        self.assertIn('plan.iter().try_for_each(|probe|', NATIVE)
        self.assertIn('device.device_fd().clone(),probe.width,probe.height,2', NATIVE)
        self.assertIn('MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED=', NATIVE)
        self.assertIn('MELANIA_NATIVE_GBM_ALL_ACTIVE_OUTPUTS_SYNCHRONIZED=', NATIVE)
        self.assertNotIn('No connected monitor mode fits the GPU memory budget', NATIVE)

    def test_active_crtc_mode_and_gpu_dimension_budget_are_hard_gates(self):
        # QEMU EDID may advertise a 5120px mode while the active CRTC is
        # 1280x800. The allocation budget must not accept the oversized
        # candidate and should prefer the live kernel mode if advertised.
        self.assertIn('let active_size=if output_sized {', NATIVE)
        self.assertIn('device.device_fd().get_crtc(crtc)', NATIVE)
        self.assertIn('info.mode().map(|mode| mode.size())', NATIVE)
        self.assertIn('let supported=conn.advertised_modes.iter().filter(', NATIVE)
        self.assertIn('validate_gbm_output_dimensions(', NATIVE)
        self.assertIn('let active_candidates=supported.iter().filter(', NATIVE)
        self.assertLess(NATIVE.index('let supported=conn.advertised_modes'),
                        NATIVE.index('let candidate=melania_native::outputs::recommended_mode_for_budget('))
        self.assertIn('width>4096 || height>4096', RENDERER)
        self.assertIn('let plan=active_output_plan.as_ref()', NATIVE)
        self.assertIn('plan.iter().try_for_each(|probe|', NATIVE)

    def test_not_installable_and_no_kms_mutation_in_probe(self):
        self.assertIn('--experimental-gbm-output', NATIVE)
        self.assertIn('experimental_drm(true,true)', NATIVE)
        self.assertIn('NATIVE_SESSION_READY=false', NATIVE)
        self.assertIn('validate_gbm_output_dimensions(width,height,frames)?;', RENDERER)
        self.assertIn('allocator.create_buffer(', RENDERER)
        self.assertIn('frame.finish()', RENDERER)
        self.assertIn('.wait().map_err(', RENDERER)
        self.assertIn('native_scanout_is_implemented() -> bool {false}', RENDERER)
        for forbidden in ('set_crtc(', 'page_flip(', 'atomic_commit(', 'create_surface('):
            self.assertNotIn(forbidden, RENDERER)

    def test_qemu_guest_executes_gpu_probe_and_host_requires_proof(self):
        self.assertIn('"$binary" --experimental-gbm-output < /dev/tty1',GUEST)
        self.assertIn('MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED=',GUEST)
        self.assertIn("report 'MELANIA_QEMU_NATIVE_GBM_OUTPUT_SIZED_PASS=true'",GUEST)
        self.assertIn("! grep -q '^MELANIA_QEMU_NATIVE_GBM_OUTPUT_SIZED_PASS=true'",HOST)
        self.assertNotIn('MELANIA_NATIVE_SESSION_READY=true',GUEST)

    def test_rust_limits_and_multi_frame_sync_are_verified(self):
        self.assertIn('fn output_sized_gpu_probe_limits_gpu_memory_and_oversized_edid()', RENDERER)
        self.assertIn('96*1024*1024', RENDERER)
        self.assertIn('for index in 0..frames {', RENDERER)
        self.assertIn('width>4096 || height>4096', RENDERER)

if __name__ == "__main__":
    unittest.main()
