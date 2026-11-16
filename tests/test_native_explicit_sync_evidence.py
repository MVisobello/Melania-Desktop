"""Host must distinguish verified explicit-sync from capability skips."""
import runpy
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
mod=runpy.run_path(str(ROOT/"scripts/verify-qemu-explicit-sync.py"))
verify=mod["verify"]
k,e,t,w,s,f=(mod[key] for key in ("KERNEL_OK","NO_EVENT","NO_TIMELINE","WIRE_OK","WIRE_SKIP","FRAMES"))

class ExplicitSyncEvidence(unittest.TestCase):
    def test_real_two_frame_success(self):
        self.assertEqual(verify("\n".join((k,*f,w))),
                         "MELANIA_VM_EXPLICIT_SYNC_RESULT=VERIFIED_TWO_GPU_FRAMES")

    def test_unsupported_capability_is_not_pass(self):
        self.assertTrue(verify(e+"\n"+s).endswith("UNSUPPORTED_EVENTFD"))
        self.assertTrue(verify(t+"\n"+s).endswith("UNSUPPORTED_TIMELINE"))

    def test_missing_partial_conflicting_or_duplicate_receipts_fail(self):
        for lines in ((),(k,w),(k,f[0],w),(k,*f,w,s),(k,k,*f,w),
                      (e,t,s),(e,*f,w),(t,s,w),(k,*f,f[1],w),(t,s,f[0])):
            with self.subTest(lines=lines), self.assertRaises(ValueError):
                verify("\n".join(lines))

    def test_marker_like_text_is_not_a_receipt(self):
        with self.assertRaises(ValueError):
            verify("echo "+k+"\n"+w)

    def test_host_calls_checker_before_general_pass(self):
        host=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
        self.assertLess(host.index("verify-qemu-explicit-sync.py"),
                        host.index("MELANIA_QEMU_NATIVE_RESULT=PASS"))
        guest=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
        self.assertLess(guest.index("explicit_client_release_timeline_not_verified_$index"),
                        guest.index("MELANIA_QEMU_EXPLICIT_WIRE_FRAME_VERIFIED=$index"))
