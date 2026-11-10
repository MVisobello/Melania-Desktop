"""QEMU-only Linux-DMABUF protocol: actual EGL import, no fake KMS frame.

These source checks supplement Rust tests; they do not replace a separate
external-client Wayland protocol/GPU/KMS acceptance test or physical VT.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
IMPORTER=(ROOT/"melania-native/src/native_client_dmabuf.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()


class QemuClientDmabufProtocol(unittest.TestCase):
    def test_smithay_global_requires_same_real_libseat_gpu_and_explicit_qemu_optin(self):
        for marker in (
            "pub mod native_client_dmabuf;",
            "delegate_dmabuf!(BridgeState);",
            "dmabuf:DmabufState,",
            "client_dmabuf_renderer:Option<GlesRenderer>",
            "client_dmabuf_imports:Vec<(wl_buffer::WlBuffer,GlesTexture)>",
            "impl DmabufHandler for BridgeState",
            "notifier.successful::<BridgeState>()",
            "notifier.failed();",
        ):
            self.assertIn(marker, LIB + BRIDGE)
        start=BRIDGE.index("let client_dmabuf_probe=")
        end=BRIDGE.index("let (sender,receiver)=UnixStream::pair()?;", start)
        setup=BRIDGE[start:end]
        for marker in (
            'MELANIA_NATIVE_CLIENT_DMABUF',
            '"YES_QEMU_VM"',
            "!integrated_kms",
            "let gpu_fd=integrated_drm.as_ref()",
            "GbmDevice::new(card)?",
            "EGLDisplay::new(gbm)",
            "GlesRenderer::new(context)",
            "native_client_dmabuf::client_import_formats(&renderer)",
            "if formats.is_empty()",
            "state.dmabuf.create_global::<BridgeState>(&handle,formats)",
            "state.client_dmabuf_renderer=Some(renderer);",
        ):
            self.assertIn(marker,setup)
        self.assertLess(setup.index("GlesRenderer::new(context)"),
                        setup.index("state.dmabuf.create_global::<BridgeState>"))
        self.assertIn("NATIVE_SESSION_READY=false", MAIN)
        self.assertNotIn("NATIVE_SESSION_READY=true", setup)

    def test_gpu_import_is_real_untrusted_fd_with_bounded_memory(self):
        for marker in (
            "MAX_CLIENT_BUFFER_BYTES:usize=16*1024*1024",
            "MAX_CLIENT_SURFACE_EDGE:usize=2048",
            "MAX_CLIENT_IMPORTS:usize=4",
            "buffer.num_planes()!=1",
            "buffer.strides().next()",
            "buffer.offsets().next()",
            "renderer.has_dmabuf_format(buffer.format())",
            "renderer.import_dmabuf(buffer,None)",
            "pub fn import_client_dmabuf(",
            "fourcc",
        ):
            # The module uses the actual Fourcc type, not a textual "fourcc".
            if marker!="fourcc":
                self.assertIn(marker,IMPORTER)
        self.assertIn("Fourcc::Xrgb8888",IMPORTER)
        self.assertIn("Fourcc::Argb8888",IMPORTER)
        self.assertIn("fn rejects_unhandled_multiplane_and_yuv_layouts()",IMPORTER)
        self.assertIn("fn rejects_bad_geometry_large_stride_and_backing_offsets()",IMPORTER)

    def test_no_fake_wayland_presentation_and_gpu_discard_on_each_loss_path(self):
        start=BRIDGE.index("impl DmabufHandler for BridgeState")
        end=BRIDGE.index("impl CompositorHandler for BridgeState",start)
        importer=BRIDGE[start:end]
        self.assertIn("self.native_presentation_guard.ticket(true).is_none()", importer)
        self.assertIn("self.lifecycle.seat_active()", importer)
        self.assertIn("self.client_dmabuf_imports.push((buffer,texture));",importer)
        for forbidden in (
            ".frame(", "frame_callback(", "publish_verified_crtc_receipt(",
            "self.presented(", "self.poll_continuous_kms(", "state.continuous_worker",
        ):
            self.assertNotIn(forbidden,importer)
        commit=BRIDGE[BRIDGE.index("if self.continuous_kms_mode {"):]
        self.assertIn("smithay::wayland::dmabuf::get_dmabuf(&buffer).is_ok()",commit)
        self.assertIn("MELANIA_NATIVE_CLIENT_DMABUF_NOT_PRESENTED=true",commit)
        self.assertIn("buffer.release();",commit)
        for marker in (
            "revoke_client_dmabuf_gpu(state);",
            "revoke_client_dmabuf_gpu(&mut state);",
            "state.client_dmabuf_imports.clear();",
            "state.client_dmabuf_renderer.take();",
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("self.client_dmabuf_imports.retain(|(known,_)|known!=buffer)",BRIDGE)
        cleanup=BRIDGE[BRIDGE.index("if integrated_kms && run_result.is_err()"):]
        self.assertLess(cleanup.index("revoke_client_dmabuf_gpu(&mut state)"),
                        cleanup.index("owner.release()"))


if __name__=="__main__":
    unittest.main()
