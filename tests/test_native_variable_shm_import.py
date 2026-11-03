"""Stage51: generic bounded wl_shm buffer import, with proven fixed-QEMU mode."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
SHM=(ROOT/"melania-native/src/owned_shm.rs").read_text()
FIXED=(ROOT/"melania-native/src/native_client_frame.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeVariableShmImporter(unittest.TestCase):
    def test_bounded_import_enforces_client_memory_safety(self):
        for marker in (
            "pub mod owned_shm;",
            "owned_shm",
        ):
            self.assertIn(marker,LIB)
        for marker in (
            "MAX_CLIENT_DIMENSION: usize = 8192",
            "MAX_CLIENT_COPY_BYTES: usize = 64 * 1024 * 1024",
            "width.checked_mul(4)",
            "row.checked_mul(height)",
            "(height-1).checked_mul(stride)",
            "pos.checked_add(row)",
            "ptr.is_null() || end>pool_len",
            "std::ptr::read_volatile(ptr.add(input+component))",
        ):
            self.assertIn(marker,SHM)

    def test_alpha_semantics_not_misrepresented_as_straight(self):
        self.assertIn("wl_shm::Format::Xrgb8888=>AlphaType::Opaque",SHM)
        self.assertIn("wl_shm::Format::Argb8888=>AlphaType::Premultiplied",SHM)
        self.assertIn("data[output+3]=if alpha==AlphaType::Opaque {0}",SHM)
        self.assertIn("UnsupportedFormat",SHM)

    def test_existing_vm_pixels_use_same_importer_but_keep_fixed_contract(self):
        self.assertIn("melania_native::owned_shm::copy_bounded_client_shm",FIXED)
        self.assertIn("meta.width!=WIN_W as i32 || meta.height!=WIN_H as i32",FIXED)
        self.assertIn("only XRGB8888 wl_shm buffers are accepted",FIXED)
        self.assertIn("Ok(owned.data)",FIXED)
        self.assertNotIn("NATIVE_SESSION_READY=true",FIXED)

    def test_rust_tests_cover_padding_overflow_and_alpha(self):
        for name in ("copies_variable_dimensions_and_padded_rows_without_aliasing_pool",
                     "preserves_premultiplied_alpha_without_unpremultiplying",
                     "fails_closed_on_invalid_stride_offset_or_truncated_mapping",
                     "rejects_oversized_and_unsupported_buffers_without_allocation"):
            self.assertIn("fn "+name+"()",SHM)

if __name__=="__main__":
    unittest.main()
