"""P0: two real libseat acquisitions must rebuild and import exported DMA-BUF FDs.

This source contract is in addition to Rust compilation and QEMU's real GPU
probe; it does NOT certify actual Wayland-client DMA-BUF or physical VT login.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RENDERER = (ROOT / "melania-native/src/renderer.rs").read_text()
NATIVE = (ROOT / "melania-native/src/main.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()


class ExportedDmabufVtImport(unittest.TestCase):
    def importer(self):
        return RENDERER.split("pub fn smoke_import_exported_dmabuf(", 1)[1].split(
            "pub fn native_scanout_is_implemented()", 1
        )[0]

    def test_actual_kernel_plane_fds_not_rust_arc_or_placeholder(self):
        path = self.importer()
        for required in (
            "validate_gbm_output_dimensions(width,height,2)?;",
            "DmabufAllocator(GbmAllocator::new(",
            "producer.create_buffer(",
            "consumer.create_buffer(",
            "GbmDevice::new(fd)",
            "EGLDisplay::new(gbm)",
            "GlesRenderer::new(context)",
            "renderer.bind(&mut source)",
            "frame.finish()",
            ".wait().map_err(",
            "Dmabuf::builder_from_buffer(&source,DmabufFlags::empty())",
            "source.handles()",
            ".zip(source.offsets()).zip(source.strides())",
            "handle.try_clone_to_owned()",
            "builder.add_plane(",
            "drop(source);",
            "renderer.has_dmabuf_format(exported.format())",
            "renderer.import_dmabuf(&exported,None)",
            "renderer.bind(&mut destination)",
            "frame.render_texture_from_to(",
        ):
            self.assertIn(required, path)
        self.assertLess(path.index("frame.finish()"),
                        path.index("handle.try_clone_to_owned()"))
        self.assertLess(path.index("drop(source);"),
                        path.index("renderer.import_dmabuf(&exported,None)"))
        self.assertLess(path.index("renderer.import_dmabuf(&exported,None)"),
                        path.index("renderer.bind(&mut destination)"))
        self.assertLess(path.index("frame.render_texture_from_to("),
                        path.rindex(".wait().map_err("))

    def test_real_vt_reacquire_runs_import_before_kernel_topology_recheck(self):
        method = NATIVE.split("fn experimental_vt_reacquire()", 1)[1].split(
            "fn experimental_drm(", 1
        )[0]
        for marker in (
            "MELANIA_NATIVE_PHYSICAL_VT_PROOF",
            "MELANIA_NATIVE_READONLY_CYCLE",
            "session.open(",
            "smoke_render_gbm_output_buffers(",
            "smoke_import_exported_dmabuf(",
            "inspect_stable_active_outputs_with_timings(&card)?",
            "session.close(fd)?",
            "MELANIA_NATIVE_VT_DUPLICATED_DMABUF_IMPORT_VERIFIED=",
        ):
            self.assertIn(marker, method)
        self.assertLess(method.index("smoke_render_gbm_output_buffers("),
                        method.index("smoke_import_exported_dmabuf("))
        self.assertLess(method.index("smoke_import_exported_dmabuf("),
                        method.index("let (after,after_timings)"))
        self.assertLess(method.index("verify_libseat_drm_primary_card("),
                        method.index("MELANIA_NATIVE_VT_DUPLICATED_DMABUF_IMPORT_VERIFIED="))
        self.assertIn("let render_gbm_each_acquire=render_gbm_each_acquire || physical_vt_proof;", method)

    def test_qemu_requires_both_real_fd_imports_and_never_claims_login(self):
        for index in (1, 2):
            self.assertIn(
                f"MELANIA_NATIVE_VT_DUPLICATED_DMABUF_IMPORT_VERIFIED={index} count=",
                GUEST,
            )
        self.assertIn("native_libseat_DRM_reopen_evidence_missing", GUEST)
        self.assertIn("MELANIA_NATIVE_VT_READONLY_QEMU_CYCLE=true", GUEST)
        self.assertIn('NATIVE_SESSION_READY=false', NATIVE)
        self.assertIn("pub fn native_scanout_is_implemented() -> bool {false}", RENDERER)
        for forbidden in ("set_crtc(", "page_flip(", "atomic_commit(", "create_surface("):
            self.assertNotIn(forbidden, RENDERER)


if __name__ == "__main__":
    unittest.main()
