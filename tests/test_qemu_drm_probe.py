#!/usr/bin/env python3
"""QEMU stage16: virtual DRM/KMS probe must be genuine, isolated and read-only."""
import contextlib
import io
from pathlib import Path
import subprocess
import unittest
from unittest import mock
import importlib.util

ROOT=Path(__file__).resolve().parents[1]
HOST=(ROOT/"scripts/run-qemu-vt-smoke.sh").read_text()
C=(ROOT/"scripts/melania-qemu-kms-probe.c").read_text()
WORKFLOW=(ROOT/".github/workflows/qemu-drm-probe.yml").read_text()
spec=importlib.util.spec_from_file_location("melania_qemu_vt_guest",ROOT/"scripts/melania-vt-guest.py")
guest=importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)

class QemuDrmProbeTests(unittest.TestCase):
    def test_host_gpu_preset_requires_isolation_and_emulates_a_gpu(self):
        for term in ('MELANIA_QEMU_GPU_MODEL', 'MELANIA_QEMU_REQUIRE_DRM',
                     'MELANIA_QEMU_TTY_SMOKE', 'YES_DISPOSABLE_QEMU', 'YES_VIRTIO_GPU',
                     'vga_device="virtio"',
                     'sha256sum -c verify.sha256',
                     'MELANIA_QEMU_KMS_READ_ONLY_PASS=true',
                     'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false'):
            self.assertIn(term,HOST)
        self.assertNotIn("accel=kvm",HOST)
        self.assertNotIn("-device vfio",HOST)
        self.assertIn("timeout --signal=TERM",HOST)
        # A shell ':' prefix consumes all following words as arguments,
        # accidentally skipping the standard-VGA guest script altogether.
        self.assertIn("MELANIA_QEMU_KMS_READ_ONLY_PASS=true",HOST)

    def test_drm_guest_code_uses_real_uapi_not_fake_stubs_or_kms_writes(self):
        for term in ('DRM_IOCTL_GET_CAP', 'DRM_IOCTL_SET_CLIENT_CAP',
                     'DRM_CLIENT_CAP_ATOMIC', 'DRM_IOCTL_MODE_GETRESOURCES',
                     'resources.count_crtcs > 0', 'resources.count_connectors > 0',
                     'MELANIA_QEMU_KMS_READ_ONLY_PASS='):
            self.assertIn(term,C)
        for forbidden in ('DRM_IOCTL_MODE_ATOMIC', 'DRM_IOCTL_MODE_SETCRTC',
                          'DRM_IOCTL_MODE_ADDFB', 'DRM_IOCTL_MODE_PAGE_FLIP',
                          'DRM_IOCTL_MODE_CREATE_DUMB', 'DRM_IOCTL_MODE_DESTROY_DUMB'):
            self.assertNotIn(forbidden,C)

    def test_missing_required_drm_aborts_before_tty_change(self):
        out=io.StringIO()
        with mock.patch.object(guest,"dmi_is_qemu",return_value=True), \
             mock.patch.object(guest,"virtual_vga_present",return_value=True), \
             mock.patch.object(guest,"drm_card_present",return_value=False), \
             mock.patch.dict(guest.os.environ,{"MELANIA_QEMU_GUEST_REQUIRE_DRM":"1"}), \\
             mock.patch.object(guest.subprocess,"run") as modprobe, \
             mock.patch.object(guest.os,"open") as opened, \
             contextlib.redirect_stdout(out):
            with self.assertRaisesRegex(RuntimeError,"did not expose"):
                guest.main()
            opened.assert_not_called()
        self.assertIn("MELANIA_QEMU_DRM_ENUMERATED=false",out.getvalue())

    def test_workflow_uses_guest_and_collects_evidence(self):
        self.assertIn("MELANIA_QEMU_REQUIRE_DRM: YES_VIRTIO_GPU",WORKFLOW)
        self.assertIn('MELANIA_QEMU_REQUIRE_DRM: YES_VIRTIO_GPU',WORKFLOW)
        self.assertIn("libdrm-dev gcc",WORKFLOW)
        self.assertIn("qemu-virtio-drm-evidence",WORKFLOW)
        self.assertIn("if: always()",WORKFLOW)

    def test_host_fails_without_manual_opt_in(self):
        r=subprocess.run(["bash",str(ROOT/"scripts/run-qemu-vt-smoke.sh")],
                         env={"PATH":"/usr/bin:/bin", "MELANIA_QEMU_REQUIRE_DRM":"YES_VIRTIO_GPU"},
                         text=True,capture_output=True)
        self.assertEqual(r.returncode,78)
        self.assertIn("MELANIA_QEMU_HOST_BLOCKED",r.stderr)

if __name__=="__main__":
    unittest.main()
