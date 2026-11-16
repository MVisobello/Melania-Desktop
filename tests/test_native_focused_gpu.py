"""Focused VM uses the same full-suite direct GPU validation, not fake receipts."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-direct-gpu-focused.sh").read_text()
CI=(ROOT/".github/workflows/qemu-direct-gpu-focused.yml").read_text()

class NativeFocusedGpu(unittest.TestCase):
    def test_shared_real_acceptance_case(self):
        case=GUEST.split("run_direct_gpu_probe() {",1)[1].split("\n}",1)[0]
        self.assertIn("MELANIA_NATIVE_DIRECT_CLIENT_GBM=YES_QEMU_VM",case)
        self.assertIn('for index in $(seq 1 12)',case)
        self.assertIn("direct_gpu_client_used_forbidden_cpu_staging",case)
        self.assertIn("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",case)
        self.assertIn("MELANIA_QEMU_DIRECT_CLIENT_GPU_TO_GBM_KMS_12_FRAMES_PASS=true",case)
        self.assertEqual(GUEST.count("\n    run_direct_gpu_probe\n")+GUEST.count("\nrun_direct_gpu_probe\n"),2)

    def test_focused_vm_keeps_real_device_guards(self):
        self.assertIn("MELANIA_QEMU_NATIVE_VM:-",HOST)
        self.assertIn("q35,accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertIn("sha256sum -c verify.sha256",HOST)
        self.assertIn("--direct-gpu-only",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_GUEST_EXIT=0",HOST)
        self.assertIn("MELANIA_QEMU_FOCUSED_DIRECT_GPU_PASS=true",HOST)
        self.assertIn("cargo build -p melania-native --bin melania-native --bin qemu-continuous-client",CI)

    def test_retained_scene_runs_independently_of_direct_case(self):
        case=GUEST.split("run_retained_gpu_probe() {",1)[1].split("\n}",1)[0]
        self.assertIn("MELANIA_NATIVE_DIRECT_GPU_MULTIWINDOW=YES_QEMU_VM",case)
        self.assertIn("MELANIA_QEMU_RETAINED_GPU_FOUR_WINDOW_CLIENT_12_ACKS=true",case)
        self.assertIn("MELANIA_NATIVE_RETAINED_GPU_RESIZE_VERIFIED=$index",case)
        self.assertIn("MELANIA_NATIVE_RETAINED_GPU_SHRINK_CLEARED=$index",case)
        self.assertIn("--retained-gpu-only",HOST)
        self.assertIn("mode: [direct, retained]",CI)
        self.assertIn("fail-fast: false",CI)
