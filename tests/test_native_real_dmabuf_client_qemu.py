"""P0: an independent Wayland client must submit actual GPU DMA-BUF FDs.

Static audits supplement (never substitute) real Smithay cargo compilation
and the isolated QEMU kernel DRM/KMS test. This test MUST NOT mark DMA-BUF
scanout complete: the compositor still rejects external client presentation.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-client.rs").read_text()
SERVER=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class RealWaylandDmabufClient(unittest.TestCase):
    def test_second_process_uses_real_gpu_and_linux_dmabuf_protocol(self):
        for marker in (
            "MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF",
            '"zwp_linux_dmabuf_v1"',
            "zwp_linux_dmabuf_v1::Event::Modifier",
            "state.dmabuf_formats.contains(&(format as u32,modifier))",
            'open("/dev/dri/renderD128")',
            'open("/dev/dri/card0")',
            "GbmAllocator::new(",
            "GbmBufferFlags::RENDERING",
            "allocator.create_buffer(",
            "EGLDisplay::new(gbm)",
            "GlesRenderer::new(context)",
            "renderer.bind(&mut image)",
            "frame.clear(",
            "frame.finish()?.wait()?",
            "dmabuf.create_params(qh,())",
            "image.handles().zip(image.offsets()).zip(image.strides())",
            "params.add(fd,plane_count as u32,offset,stride",
            "params.create_immed(",
            "probe.attach(Some(&imported),0,0)",
            "probe.commit()",
            "while !state.buffer_released",
            "if state.frame_done_count!=0",
            "MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_RELEASED_NO_KMS=true",
        ):
            self.assertIn(marker,CLIENT)
        imp=CLIENT.split("fn exercise_real_client_dmabuf(",1)[1].split(
            "fn main()->Result",1)[0]
        self.assertLess(imp.index("frame.finish()?.wait()?"),
                        imp.index("params.create_immed("))
        self.assertLess(imp.index("params.create_immed("),
                        imp.index("probe.commit()"))
        self.assertLess(imp.index("probe.commit()"),
                        imp.index("imported.destroy()"))

    def test_qemu_guest_observes_real_import_but_forbids_fake_ack(self):
        begin=GUEST.index("# ONE-PROCESS native integration proof:")
        end=GUEST.index("# Sustain 24 successive real Wayland/client buffer",begin)
        scenario=GUEST[begin:end]
        for marker in (
            "MELANIA_NATIVE_CLIENT_DMABUF=YES_QEMU_VM",
            "MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF=YES_QEMU_VM",
            "MELANIA_NATIVE_QEMU_CLIENT_DMABUF_GPU_GLOBAL_READY=true",
            "MELANIA_NATIVE_CLIENT_DMABUF_GPU_TEXTURE_IMPORTED=true",
            "MELANIA_NATIVE_CLIENT_DMABUF_NOT_PRESENTED=true",
            "MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_RELEASED_NO_KMS=true",
            "MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12",
            "MELANIA_NATIVE_CONTINUOUS_KMS_ACK=",
            "integrated_real_client_DMA_BUF_import_or_fail_closed_presentation_missing",
        ):
            self.assertIn(marker,scenario)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertIn("buffer.release();",SERVER)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_NOT_PRESENTED=true",SERVER)
        self.assertIn("self.client_dmabuf_imports.retain",SERVER)
        self.assertEqual(GUEST.count("# Stage48:"),1)
        self.assertEqual(GUEST.count("# ONE-PROCESS native integration proof:"),1)


if __name__ == "__main__":
    unittest.main()
