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
        # VirtIO's dumb BO allocation ioctl is not allowed on render
        # nodes. Require a second-process primary FD, never a renderD128
        # fallback or a client DRM master/modeset request.
        self.assertNotIn('open("/dev/dri/renderD128")',CLIENT)
        self.assertNotIn("set_master(",CLIENT)
        imp=CLIENT.split("fn exercise_real_client_dmabuf(",1)[1].split(
            "fn main()->Result",1)[0]
        self.assertLess(imp.index("frame.finish()?.wait()?"),
                        imp.index("params.create_immed("))
        self.assertLess(imp.index("params.create_immed("),
                        imp.index("probe.commit()"))
        self.assertLess(imp.index("probe.commit()"),
                        imp.index("imported.destroy()"))

    def test_real_gpu_fd_y_invert_rejected_over_wayland_without_kms_ack(self):
        # The negative case must use the real GBM allocation and the
        # asynchronous params.create/failed event, not a local-only predicate.
        source=CLIENT.split("fn exercise_real_client_dmabuf(",1)[1].split(
            "fn main()->Result",1)[0]
        for marker in (
            'MELANIA_QEMU_CLIENT_REJECT_Y_INVERT',
            'image.handles().zip(image.offsets()).zip(image.strides())',
            'invalid.add(fd,planes as u32,offset,stride,',
            'invalid.create(W as i32,H as i32,format as u32,',
            'zwp_linux_buffer_params_v1::Flags::YInvert',
            'queue.roundtrip(state)?;',
            'if !state.invalid_invert_failed || state.invalid_invert_created',
            'state.frame_done_count!=old_callback_count',
            'MELANIA_QEMU_REAL_CLIENT_Y_INVERT_REJECTED=true',
        ):
            self.assertIn(marker,source)
        self.assertLess(source.index('invalid.create('),
                        source.index('queue.roundtrip(state)?;',source.index('invalid.create(')))
        self.assertIn('zwp_linux_buffer_params_v1::Event::Failed=>state.invalid_invert_failed=true',CLIENT)
        self.assertIn('zwp_linux_buffer_params_v1::Event::Created{buffer}',CLIENT)
        scenario=GUEST[GUEST.index('# ONE-PROCESS native integration proof:'):
                       GUEST.index('# Sustain 24 successive real Wayland/client buffer')]
        for marker in (
            'MELANIA_QEMU_CLIENT_REJECT_Y_INVERT=YES_QEMU_VM',
            'MELANIA_QEMU_REAL_CLIENT_Y_INVERT_REJECTED=true',
            'MELANIA_NATIVE_CLIENT_DMABUF_IMPORT_REFUSED=client DMA-BUF requires progressive non-inverted flags',
        ):
            self.assertIn(marker,scenario)
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false',GUEST)

    def test_real_gpu_fd_short_stride_rejected_by_server_and_client(self):
        source=CLIENT.split("fn exercise_real_client_dmabuf(",1)[1].split(
            "fn main()->Result",1)[0]
        for marker in (
            'MELANIA_QEMU_CLIENT_REJECT_SHORT_STRIDE',
            'image.handles().zip(image.offsets()).zip(image.strides())',
            'invalid.add(fd,planes as u32,offset,4,',
            'zwp_linux_buffer_params_v1::Flags::empty()',
            'if !state.invalid_invert_failed || state.invalid_invert_created',
            'state.frame_done_count!=old_callback_count',
            'MELANIA_QEMU_REAL_CLIENT_SHORT_STRIDE_REJECTED=true',
        ):
            self.assertIn(marker,source)
        start=source.index('MELANIA_QEMU_CLIENT_REJECT_SHORT_STRIDE')
        end=source.index('MELANIA_QEMU_REAL_CLIENT_SHORT_STRIDE_REJECTED=true')
        negative=source[start:end]
        self.assertIn('invalid.create(',negative)
        self.assertIn('queue.roundtrip(state)?;',negative)
        self.assertNotIn('probe.attach(',negative)
        self.assertNotIn('probe.commit()',negative)
        scenario=GUEST[GUEST.index('# ONE-PROCESS native integration proof:'):
                       GUEST.index('# Sustain 24 successive real Wayland/client buffer')]
        for marker in (
            'MELANIA_QEMU_CLIENT_REJECT_SHORT_STRIDE=YES_QEMU_VM',
            'MELANIA_QEMU_REAL_CLIENT_SHORT_STRIDE_REJECTED=true',
            'MELANIA_NATIVE_CLIENT_DMABUF_IMPORT_REFUSED=client DMA-BUF stride outside bounded packed-RGB layout',
        ):
            self.assertIn(marker,scenario)

    def test_real_gpu_fd_out_of_bounds_offset_rejected_without_frame_ack(self):
        source=CLIENT.split("fn exercise_real_client_dmabuf(",1)[1].split(
            "fn main()->Result",1)[0]
        begin=source.index('MELANIA_QEMU_CLIENT_REJECT_OOB_OFFSET')
        end=source.index('MELANIA_QEMU_REAL_CLIENT_OOB_OFFSET_REJECTED=true')
        negative=source[begin:end]
        for marker in (
            'image.handles().zip(image.offsets()).zip(image.strides())',
            'invalid.add(fd,planes as u32,u32::MAX,stride,',
            'invalid.create(',
            'queue.roundtrip(state)?;',
            'if !state.invalid_invert_failed || state.invalid_invert_created',
            'state.frame_done_count!=old_callback_count',
        ):
            self.assertIn(marker,negative)
        self.assertNotIn('probe.attach(',negative)
        self.assertNotIn('probe.commit()',negative)
        scenario=GUEST[GUEST.index('# ONE-PROCESS native integration proof:'):
                       GUEST.index('# Sustain 24 successive real Wayland/client buffer')]
        for marker in (
            'MELANIA_QEMU_CLIENT_REJECT_OOB_OFFSET=YES_QEMU_VM',
            'MELANIA_QEMU_REAL_CLIENT_OOB_OFFSET_REJECTED=true',
            'MELANIA_NATIVE_CLIENT_DMABUF_IMPORT_REFUSED=client DMA-BUF unbounded plane offset or backing extent',
        ):
            self.assertIn(marker,scenario)

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
