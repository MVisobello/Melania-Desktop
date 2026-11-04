"""One-process integration: Wayland server and real DRM/KMS presenter share a PID.

These are source-level checks. Only the isolated QEMU guest may prove the
actual wl_shm -> KMS framebuffer -> kernel CRTC -> rollback sequence.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
MAIN = (ROOT / "melania-native/src/main.rs").read_text()
BRIDGE = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
RECEIVER = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()

class OneProcessWaylandKmsContract(unittest.TestCase):
    def test_same_native_executable_contains_wayland_and_kms(self):
        self.assertIn('mod qemu_continuous_receiver;', MAIN)
        self.assertIn('mod wayland_bridge;', MAIN)
        self.assertIn('pub(crate) fn run_embedded(stream:UnixStream,card:File)', RECEIVER)
        self.assertIn('run_stream(stream,card)', RECEIVER)
        self.assertIn('qemu_continuous_receiver::run_embedded(', BRIDGE)

    def test_no_external_kms_process_required_for_embedded_mode(self):
        self.assertIn('UnixStream::pair()?', BRIDGE)
        self.assertIn('ContinuousKmsWorker::spawn(sender)', BRIDGE)
        self.assertIn('if live_stream && !integrated_kms', BRIDGE)
        self.assertIn('MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true', BRIDGE)
        self.assertIn('MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS={}', BRIDGE)
        self.assertIn('session.open(', BRIDGE)
        self.assertIn('Path::new("/dev/dri/card0")', BRIDGE)
        self.assertIn('struct OwnedSeatCard', BRIDGE)
        self.assertIn('self.session.close(fd)', BRIDGE)
        self.assertIn('MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true', BRIDGE)
        self.assertIn('std::fs::File::from(presenter_fd)', BRIDGE)

    def test_worker_completion_is_awaited_and_failure_is_fatal(self):
        self.assertIn('let outcome=join.join()', BRIDGE)
        self.assertIn('outcome.map_err(|e|format!("in-process KMS presenter failed: {e}"))?', BRIDGE)
        self.assertLess(BRIDGE.index('let outcome=join.join()'),
                        BRIDGE.index('MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS={}'))

    def test_all_wayland_errors_unwind_kms_before_libseat_fd_close(self):
        self.assertIn('let run_result=(||->Result<(),Box<dyn Error>> {', BRIDGE)
        self.assertIn('if integrated_kms && run_result.is_err()', BRIDGE)
        cleanup=BRIDGE[BRIDGE.index('if integrated_kms && run_result.is_err()'):]
        self.assertLess(cleanup.index('state.continuous_worker.take()'),
                        cleanup.index('join.join()'))
        self.assertLess(cleanup.index('join.join()'),
                        cleanup.index('owner.release()'))
        self.assertIn('MELANIA_NATIVE_IN_PROCESS_KMS_ERROR_CLEANUP_ATTEMPTED=true', cleanup)
        self.assertTrue(cleanup.index('run_result') >= 0)

    def test_qemu_only_optin_and_original_login_block_remain(self):
        for required in (
            'MELANIA_NATIVE_IN_PROCESS_KMS',
            'MELANIA_QEMU_CONTINUOUS_KMS',
            'YES_DISPOSABLE_QEMU',
            'MELANIA_QEMU_CONTINUOUS_FRAMES',
        ):
            self.assertIn(required, BRIDGE + RECEIVER)
        self.assertIn('guest_only()?;', RECEIVER)
        self.assertIn('NATIVE_SESSION_READY=false', BRIDGE)

    def test_vm_runs_real_client_without_second_kms_binary(self):
        start=GUEST.index('# ONE-PROCESS native integration proof:')
        end=GUEST.index("report 'MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_12_FRAMES_PASS=true'",start)
        scenario=GUEST[start:end]
        self.assertIn('"$continuous_server" --experimental-wayland-seat', scenario)
        self.assertIn('"$continuous_client"', scenario)
        self.assertNotIn('"$continuous_kms"', scenario)
        # A JS replacement accidentally expanded "$'" and duplicated all
        # following QEMU scenarios once. Keep the guest script intact.
        self.assertEqual(GUEST.count("# Stage48:"), 1)
        self.assertEqual(GUEST.count("two_continuous_client=/mnt/melania-native/melania-two-continuous-client"), 1)
        self.assertIn(
            "grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' \"$integrated_server_log\" ||",
            scenario,
        )
        self.assertIn('MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM', scenario)
        for receipt in (
            'MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12',
            'MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true',
            'MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS=12',
        ):
            self.assertIn(receipt,scenario)

if __name__ == "__main__":
    unittest.main()
