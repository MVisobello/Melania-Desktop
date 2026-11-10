"""P0 isolated VM: an actual second-process DMA-BUF GPU frame reaches KMS.

These contract checks accompany full Rust compile and QEMU/virtio DRM
tests; source markers alone cannot verify EGL, GPU readback or CRTC ACK.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GPU=(ROOT/"melania-native/src/native_client_dmabuf.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-client.rs").read_text()
DRM=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class RealClientDmabufToKms(unittest.TestCase):
    def test_client_produces_gpu_image_and_waits_for_real_kms(self):
        for marker in (
            "MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS",
            "frame.finish()?.wait()?",
            "params.create_immed(",
            "probe.frame(qh,())",
            "while !state.buffer_released ||",
            "(gpu_kms && state.frame_done_count!=1)",
            "MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME1_KMS_ACKED=true",
            "for index in 1..=total",
            "if gpu_first && index==1 {continue;}",
            "MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=1",
        ):
            self.assertIn(marker,CLIENT)

    def test_egl_readback_converts_bottom_up_rgba_with_owned_bytes(self):
        for marker in (
            "readback_qemu_client_bgra",
            "renderer.copy_texture(texture,region,Fourcc::Abgr8888)",
            "renderer.map_texture(&mapping)",
            "mapping.flipped()",
            "convert_rgba_readback_to_bgra",
            "let mut pixels=vec![0;len]",
            "dst.copy_from_slice(&[src[2],src[1],src[0],0]);",
            "fn readback_swizzles_rgba_into_opaque_bgra_and_flips_bottom_up()",
        ):
            self.assertIn(marker,GPU)

    def test_gpu_buffer_released_only_after_real_drm_ack_and_ticket(self):
        for marker in (
            "MELANIA_NATIVE_CLIENT_DMABUF_KMS",
            "self.continuous_frame_count!=0",
            "readback_qemu_client_bgra(renderer,texture)",
            "self.multi_scene.upsert(key,pixels)",
            "self.continuous_client_ledger.stage(surface.clone(),buffer)",
            "self.submit_continuous_frame()",
            "MELANIA_NATIVE_CLIENT_DMABUF_GPU_PIXELS_READBACK=360x224",
            "MELANIA_NATIVE_CLIENT_DMABUF_KMS_ACKED={}",
        ):
            self.assertIn(marker,BRIDGE)
        ack=BRIDGE.split("fn poll_continuous_kms(&mut self)",1)[1].split(
            "fn begin_generation_proof(",1)[0]
        self.assertLess(ack.index("if !self.native_presentation_guard.accepts("),
                        ack.index("for completed in self.continuous_client_ledger.acknowledged(clients)"))
        self.assertLess(ack.index("if !self.continuous_redraw.presented(planned)"),
                        ack.index("completed.resource.release();"))
        self.assertLess(ack.index("completed.resource.release();"),
                        ack.index("MELANIA_NATIVE_CLIENT_DMABUF_KMS_ACKED={}"))

    def test_client_gpu_texture_is_owned_through_kms_ack_then_dropped_before_release(self):
        ack=BRIDGE.split("fn poll_continuous_kms(&mut self)",1)[1].split(
            "fn begin_generation_proof(",1)[0]
        before=ack.index("if !self.native_presentation_guard.accepts(")
        checked=ack.index('return Err("GPU DMA-BUF KMS ACK has no retained EGL client import"')
        drop=ack.index("self.client_dmabuf_imports.retain(|(known,_)|")
        callback=ack.index("complete_presented_frame(",drop)
        release=ack.index("completed.resource.release();",callback)
        marker=ack.index("MELANIA_NATIVE_CLIENT_DMABUF_KMS_ACKED={}",release)
        self.assertLess(before,checked)
        self.assertLess(checked,drop)
        self.assertLess(drop,callback)
        self.assertLess(callback,release)
        self.assertLess(release,marker)
        self.assertIn("any(|(known,_)|known==&completed.resource)",ack)
        self.assertIn("let required_gpu_client=self.continuous_frame_count==1",ack)
        self.assertIn("if required_gpu_client && !is_gpu_client",ack)
        missing=BRIDGE.split(
            'eprintln!("MELANIA_NATIVE_CLIENT_DMABUF_KMS_RENDERER_MISSING=true");',1
        )[1].split("return;",1)[0]
        self.assertLess(
            missing.index("self.client_dmabuf_imports.retain("),
            missing.index("buffer.release();")
        )
        self.assertLess(
            ack.index("if required_gpu_client && !is_gpu_client"),
            ack.index("complete_presented_frame(",drop)
        )

    def test_kernel_checks_actual_gpu_color_and_fb_id_before_ack(self):
        for marker in (
            "MELANIA_NATIVE_CLIENT_DMABUF_KMS",
            "let center=((120+112)*W+(160+180))*4;",
            "MELANIA_RUST_GPU_CLIENT_FRAME1_PIXELS_IN_DRM=true",
            "MELANIA_RUST_GPU_CLIENT_DMABUF_KMS_FB_ID_ACK=1",
            "card.set_crtc(crtc_handle,Some(framebuffer)",
        ):
            self.assertIn(marker,DRM)
        self.assertLess(
            DRM.index("MELANIA_RUST_GPU_CLIENT_FRAME1_PIXELS_IN_DRM=true"),
            DRM.index("card.set_crtc(crtc_handle,Some(framebuffer)")
        )
        self.assertLess(
            DRM.index('stream.write_all(&ack)?'),
            DRM.index("MELANIA_RUST_GPU_CLIENT_DMABUF_KMS_FB_ID_ACK=1")
        )

    def test_guest_requires_GPU_kms_plus_11_shm_but_preserves_old_reject(self):
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_NOT_PRESENTED=true",GUEST)
        self.assertIn("MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_RELEASED_NO_KMS=true",GUEST)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_KMS=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS=YES_QEMU_VM",GUEST)
        self.assertIn(
            "MELANIA_QEMU_EXTERNAL_CLIENT_GPU_DMA_BUF_TO_REAL_KMS_12_FRAMES_PASS=true",GUEST)
        self.assertIn("for index in $(seq 1 12); do",GUEST)
        self.assertIn("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertEqual(GUEST.count("# ONE-PROCESS native integration proof:"),1)
        self.assertEqual(GUEST.count("# Stage48:"),1)


if __name__=="__main__":
    unittest.main()
