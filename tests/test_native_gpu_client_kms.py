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
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class RealClientDmabufToKms(unittest.TestCase):
    def test_client_produces_gpu_image_and_waits_for_real_kms(self):
        for marker in (
            "MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS",
            "frame.finish()?.wait()?",
            "params.create_immed(",
            "probe.frame(qh,())",
            "while !state.buffer_released ||",
            "(gpu_kms && state.frame_done_count!=gpu_index)",
            "MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME1_KMS_ACKED=true",
            "for index in 1..=total",
            "if gpu_first && index==1 {continue;}",
            "if gpu_second && index==2 {continue;}",
            "MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME2_KMS_ACKED=true",
            "MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=1",
        ):
            self.assertIn(marker,CLIENT)

    def test_driver_advertised_modifiers_are_bounded_and_deterministic(self):
        # The actual Smithay GPU registry must invoke the prioritized,
        # bounded format list: Rust unit tests test order/dedup, while
        # the full guest proves its ARGB8888 LINEAR client can bind and
        # present two different buffers through actual DRM/KMS.
        for marker in (
            "MAX_ADVERTISED_CLIENT_FORMATS:usize=64",
            "fn ordered_advertised_formats(",
            "candidates.sort_by_cached_key(",
            "format.modifier!=Modifier::Linear",
            "candidates.dedup();",
            "candidates.truncate(MAX_ADVERTISED_CLIENT_FORMATS);",
            "ordered_advertised_formats(renderer.dmabuf_formats().iter().copied())",
            "renderer.has_dmabuf_format(buffer.format())",
            "fn advertised_linear_modifiers_are_deterministic_and_not_dropped()",
            "fn advertised_modifier_priority_never_fabricates_gpu_support()",
        ):
            self.assertIn(marker,GPU)
        self.assertIn("client_import_formats(&renderer)",BRIDGE)
        self.assertIn("MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME2_KMS_ACKED=true",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)

    def test_egl_readback_converts_bottom_up_rgba_with_owned_bytes(self):
        for marker in (
            "readback_qemu_client_bgra",
            "renderer.copy_texture(texture,region,Fourcc::Abgr8888)",
            "renderer.map_texture(&mapping)",
            "mapping.flipped()",
            "convert_rgba_readback_to_bgra",
            "let mut pixels=vec![0;len]",
            "over_opaque_background(src[2],QEMU_SCENE_BGRA[0],src[3])",
            "over_opaque_background(src[1],QEMU_SCENE_BGRA[1],src[3])",
            "over_opaque_background(src[0],QEMU_SCENE_BGRA[2],src[3])",
            "fn imported_argb_premultiplied_alpha_blends_over_real_kms_background()",
            "fn readback_swizzles_rgba_into_opaque_bgra_and_flips_bottom_up()",
        ):
            self.assertIn(marker,GPU)

    def test_real_gpu_argb_alpha_is_verified_by_kernel_presenter(self):
        self.assertIn("Color32F::new(0.165,0.09,0.365,0.5)",CLIENT)
        self.assertIn("Rectangle::from_loc_and_size((20,20),(48,36))",CLIENT)
        self.assertIn("MELANIA_RUST_GPU_CLIENT_ALPHA_BLENDED_IN_DRM=true",DRM)
        self.assertIn("QEMU_SCENE_BGRA:[u8;3]=[58,33,27]",GPU)
        self.assertIn("let alpha_sample=((120+(224-1-40))*W+(160+40))*4;",DRM)
        self.assertIn("let opaque_sample=((120+40)*W+(160+40))*4;",DRM)
        self.assertLess(
            DRM.index("MELANIA_RUST_GPU_CLIENT_ALPHA_BLENDED_IN_DRM=true"),
            DRM.index("card.set_crtc(crtc_handle,Some(framebuffer)")
        )

    def test_gpu_buffer_released_only_after_real_drm_ack_and_ticket(self):
        for marker in (
            "MELANIA_NATIVE_CLIENT_DMABUF_KMS",
            "qemu_gpu_frame_allowed(",
            "MELANIA_NATIVE_CLIENT_DMABUF_DOUBLE_GPU",
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

    def test_two_different_gpu_dmabufs_present_before_ten_shm_frames(self):
        self.assertIn("for gpu_index in 1..=gpu_frames",CLIENT)
        self.assertIn("let mut image=allocator.create_buffer(",CLIENT)
        self.assertLess(
            CLIENT.index("for gpu_index in 1..=gpu_frames"),
            CLIENT.index("let mut image=allocator.create_buffer(")
        )
        self.assertIn("Color32F::new(0.12,0.64,0.27,1.0)",CLIENT)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_GPU_FRAME2_PIXELS_READBACK=360x224",BRIDGE)
        self.assertIn("self.client_dmabuf_imports.retain(|(known,_)|",BRIDGE)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_KMS_ACKED={}",BRIDGE)
        self.assertIn("MELANIA_RUST_GPU_CLIENT_FRAME2_PIXELS_IN_DRM=true",DRM)
        self.assertIn("MELANIA_RUST_GPU_CLIENT_DMABUF_KMS_FB_ID_ACK=2",DRM)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_KMS_ACKED=2",GUEST)
        self.assertIn("if gpu_second && index==2 {continue;}",CLIENT)

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
        self.assertIn("qemu_gpu_frame_required(",ack)
        self.assertIn("self.continuous_frame_count,double_gpu",ack)
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
            "MELANIA_RUST_GPU_CLIENT_FRAME2_PIXELS_IN_DRM=true",
            "MELANIA_RUST_GPU_CLIENT_DMABUF_KMS_FB_ID_ACK=2",
            "MELANIA_RUST_GPU_CLIENT_ALPHA_BLENDED_IN_DRM=true",
            # GLES scissor uses bottom-origin coordinates; after readback
            # the alpha patch is at y=224-1-40, not the opaque y=40.
            "let alpha_sample=((120+(224-1-40))*W+(160+40))*4;",
            "let opaque_sample=((120+40)*W+(160+40))*4;",
            "GPU alpha orientation control was not opaque",
            "GPU client premultiplied alpha did not blend into DRM scanout",
            "MELANIA_RUST_GPU_CLIENT_DMABUF_KMS_FB_ID_ACK=1",
            "card.set_crtc(crtc_handle,Some(framebuffer)",
        ):
            self.assertIn(marker,DRM)
        # Both the transparent patch and the independent opaque control
        # must be checked before declaring GPU pixels verified in real KMS.
        self.assertLess(
            DRM.index("let opaque_sample=((120+40)*W+(160+40))*4;"),
            DRM.index("MELANIA_RUST_GPU_CLIENT_ALPHA_BLENDED_IN_DRM=true")
        )
        self.assertLess(
            DRM.index("let alpha_sample=((120+(224-1-40))*W+(160+40))*4;"),
            DRM.index("MELANIA_RUST_GPU_CLIENT_ALPHA_BLENDED_IN_DRM=true")
        )
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
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_DOUBLE_GPU=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_DOUBLE_GPU=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME2_KMS_ACKED=true",GUEST)
        self.assertIn("MELANIA_RUST_GPU_CLIENT_FRAME2_PIXELS_IN_DRM=true",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS=YES_QEMU_VM",GUEST)
        self.assertIn(
            "MELANIA_QEMU_EXTERNAL_CLIENT_GPU_DMA_BUF_TO_REAL_KMS_12_FRAMES_PASS=true",GUEST)
        self.assertIn("for index in $(seq 1 12); do",GUEST)
        self.assertIn("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertEqual(GUEST.count("# ONE-PROCESS native integration proof:"),1)
        self.assertEqual(GUEST.count("# Stage48:"),1)
        # Authenticate the historical QMP-driven windows *first*.
        # Moving this extra DRM test ahead of those observers can silently
        # exhaust their deadlines before the real tablet press arrives.
        self.assertGreater(
            GUEST.index("# P0: NEW independent QEMU DRM session."),
            GUEST.index("report 'MELANIA_QEMU_REAL_LAYER_SHELL_12_KMS_PASS=true'"),
        )
        # Allow the separate GPU session a bounded time budget, while keeping
        # the original QMP observer assertions and screenshot requirements.
        self.assertIn("timeout --signal=TERM --kill-after=10s 2050s",HOST)
        self.assertIn("timeout-minutes: 50",CI)
        self.assertIn("qmp-authenticated-resize.log",HOST)
        self.assertIn("authenticated-resize-frame2.ppm",HOST)


if __name__=="__main__":
    unittest.main()
