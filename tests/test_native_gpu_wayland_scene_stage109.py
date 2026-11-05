"""Stage109: end-to-end real Wayland scene -> GPU GBM KMS contract.

This source-level test is intentionally not hardware evidence. The QEMU
guest is required to run real Wayland clients and check real DRM FB_ID.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GPU=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class NativeGpuWaylandScene109(unittest.TestCase):
    def test_gpu_input_is_actual_wayland_owned_composition_not_test_color(self):
        body=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        for marker in (
            'stream.read_exact(&mut header)?',
            'b"MLF1"',
            'stream.read_exact(&mut pixels)?',
            'renderer.import_memory(',
            'Fourcc::Xrgb8888',
            'frame.render_texture_from_to(',
            'frame.finish()?.wait()?',
            'GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT',
            'framebuffer_from_dmabuf(&card,&gbm,&a,true,true)',
            'framebuffer_from_dmabuf(&card,&gbm,&b,true,true)',
        ):
            self.assertIn(marker,body)
        self.assertIn('self.continuous_client_ledger.take_for_presentation()',BRIDGE)
        self.assertIn('self.multi_scene.composite()',BRIDGE)

    def test_kernel_event_and_scanout_must_precede_client_ack(self):
        body=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        for marker in (
            'flip_ledger.begin_flip()', 'PageFlipFlags::EVENT',
            'wait_for_kms_page_flip_routed(', 'flip_ledger.complete_vm_flip(vblank)',
            'wait_for_scanout(&card,crtc,connector,framebuffer)?',
            'stream.write_all(&ack)?', 'rollback.restore()?',
        ):
            self.assertIn(marker,body)
        self.assertLess(body.index('wait_for_kms_page_flip_routed('),
                        body.index('wait_for_scanout(&card,crtc,connector,framebuffer)?'))
        self.assertLess(body.index('wait_for_scanout(&card,crtc,connector,framebuffer)?'),
                        body.index('stream.write_all(&ack)?'))
        self.assertLess(body.index('rollback.restore()?;'),
                        body.index('stream.write_all(&ack)?'))

    def test_explicit_qemu_only_backend_selection_and_legacy_fallback(self):
        section=KMS[KMS.index('pub(crate) fn run_embedded('):
                    KMS.index('fn run_stream(',KMS.index('pub(crate) fn run_embedded('))]
        self.assertIn('guest_only()?;',section)
        self.assertIn('MELANIA_NATIVE_IN_PROCESS_KMS',section)
        self.assertIn('MELANIA_NATIVE_GPU_WAYLAND_KMS',section)
        self.assertIn('Some("YES_DISPOSABLE_QEMU")',section)
        self.assertIn('run_stream(stream,card)',section)
        self.assertIn('qemu_wayland_gpu_scene_stream(',section)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAMES',section)
        self.assertIn('MELANIA_NATIVE_QEMU_FRAME_TARGET',section)
        self.assertIn('NATIVE_SESSION_READY=false',GPU)

    def test_real_guest_requires_24_wayland_clients_and_all_gpu_kernel_frames(self):
        begin=GUEST.index('# Stage109: prove a real Smithay Wayland client-owned wl_shm scene')
        end=GUEST.index("MELANIA_QEMU_IN_PROCESS_WAYLAND_GBM_24_FRAMES_PASS=true",begin)
        scenario=GUEST[begin:end]
        self.assertIn('MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU',scenario)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24',scenario)
        self.assertIn('seq 1 24',scenario)
        self.assertIn('MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=',scenario)
        self.assertIn('MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=',scenario)
        self.assertIn('MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=23',scenario)
        self.assertIn('MELANIA_NATIVE_GPU_WAYLAND_24_KMS_FRAMES_PASS=true',scenario)
        self.assertIn('MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true',scenario)
        self.assertIn('MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true',scenario)

if __name__=="__main__":
    unittest.main()
