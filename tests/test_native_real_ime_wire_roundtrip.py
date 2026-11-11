"""Cross-process Wayland IME acceptance remains a real guest proof.

This static audit only ensures that Rust and the QEMU guest enforce the
two-process test; only the complete Rust build plus guest run can establish
that UTF-8 text traversed Smithay's real protocol dispatch.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUST = (ROOT/"melania-native/src/bin/qemu-ime-roundtrip.rs").read_text()
GUEST = (ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST = (ROOT/"scripts/run-qemu-melania-native.sh").read_text()
BRIDGE = (ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CI = (ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class RealImeWireRoundtrip(unittest.TestCase):
    def test_three_real_wayland_clients_share_no_fake_registry(self):
        for marker in (
            "Connection::connect_to_env()",
            "registry.bind(name, version.min(1), qh, ())",
            "zwp_input_method_manager_v2",
            "zwp_text_input_manager_v3",
            "state.unexpected_privileged_ime_global = true",
            "return Err(\"untrusted Wayland PID could see input-method-v2 manager\"",
            "MELANIA_QEMU_UNTRUSTED_IME_GLOBAL_REJECTED=true",
            "MELANIA_QEMU_TRUSTED_IME_MANAGER_BOUND=true",
        ):
            self.assertIn(marker,RUST)
        self.assertNotIn("MELANIA_QEMU_REAL_IME_UTF8_ROUNDTRIP=true",RUST)
    def test_ime_utf8_transfer_is_real_protocol_committed(self):
        for marker in (
            'const TEXT: &str = "Procida è già connessa";',
            "mgr.get_input_method(&seat, &qh, ())",
            "mgr.get_text_input(&seat, &qh, ())",
            "wm.get_xdg_surface(&surface, &qh, ())",
            "shm.create_pool(file.as_fd()",
            "surface.attach(Some(&buffer)",
            "zwp_text_input_v3::Event::Enter",
            "zwp_text_input_v3::Event::Leave",
            "state.editor_surface = Some(surface.clone())",
            "state.editor_surface.as_ref() != Some(&surface)",
            "IME text delivered without active actual editor focus",
            "zwp_input_method_v2::Event::Activate",
            "zwp_input_method_v2::Event::Done",
            "method.commit_string(TEXT.into())",
            "method.commit(serial)",
            "zwp_text_input_v3::Event::CommitString",
            "state.text_received.as_deref() != Some(TEXT)",
            "MELANIA_QEMU_REAL_EDITOR_IME_UTF8_ROUNDTRIP=true",
            "zwp_input_method_v2::Event::Deactivate",
        ):
            self.assertIn(marker,RUST)
        self.assertLess(RUST.index("method.commit_string(TEXT.into())"),
                        RUST.index("method.commit(serial)"))
    def test_guest_pass_requires_real_own_pid_and_both_connections(self):
        for marker in (
            'ime_client_pid=$!',
            'MELANIA_NATIVE_TRUSTED_IME_PID="$ime_client_pid"',
            'MELANIA_NATIVE_TEXT_INPUT_IME=YES_QEMU_VM',
            'MELANIA_QEMU_IME_ROUNDTRIP=YES_QEMU_VM',
            '"$ime_probe" ime "$ime_target"',
            '"$ime_probe" editor',
            '"$ime_probe" intruder',
            "fatal 'real_IME_editor_cross_process_utf8_failed'",
            "fatal 'untrusted_client_gained_IM_protocol_global'",
            "MELANIA_QEMU_REAL_EDITOR_IME_UTF8_ROUNDTRIP=true",
            "MELANIA_QEMU_UNTRUSTED_IME_GLOBAL_REJECTED=true",
            "MELANIA_QEMU_REAL_IM_V2_TO_TI_V3_UTF8_WIRE_ROUNDTRIP_PASS=true",
        ):
            self.assertIn(marker,GUEST)
        self.assertIn('melania-ime-roundtrip=$ime_probe',HOST)
        self.assertIn('test -x target/debug/qemu-ime-roundtrip',CI)
        self.assertGreater(
            GUEST.index("# P0 text-input-v3 + input-method-v2"),
            GUEST.index("MELANIA_QEMU_EXTERNAL_CLIENT_GPU_DMA_BUF_TO_REAL_KMS_12_FRAMES_PASS=true")
        )
    def test_server_hides_global_from_every_other_pid_and_is_not_login(self):
        for marker in (
            "MELANIA_NATIVE_TEXT_INPUT_IME",
            "MELANIA_NATIVE_TRUSTED_IME_PID",
            "libc::SO_PEERCRED",
            "kernel_peer_pid=u32::try_from(credentials.pid).ok()",
            "client.get_data::<BridgeClientState>()",
            "trusted_ime_peer(data.kernel_peer_pid,pid)",
            "kernel_peer_pid,",
            "fn trusted_ime_peer(kernel_peer_pid:Option<u32>,allowed_pid:u32)->bool",
            'MELANIA_NATIVE_SESSION_READY=false',
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("MELANIA_NATIVE_WAYLAND_RENDER_READY=false",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertNotIn("client.get_credentials(&identity_handle)",BRIDGE)
        self.assertIn("fn only_exact_kernel_socket_pid_is_privileged()",BRIDGE)

    def test_server_finishes_only_after_real_kernel_peer_disconnects(self):
        # Only Linux SO_PEERCRED can designate the trusted input method.
        # The Rust lifecycle receipt alone is never a fake text commit.
        for marker in (
            "struct ImeGuestLifecycle",
            "trusted_disconnects: AtomicU32",
            "ordinary_disconnects: AtomicU32",
            "fn disconnected(&self,_:ClientId,_:DisconnectReason)",
            "tracker.all_three_disconnected(",
            "ime_trusted_connections,ime_untrusted_connections",
            "MELANIA_NATIVE_QEMU_IME_THREE_PEER_DISCONNECTS_VERIFIED=true",
            "trusted_ime_pid.is_none() && !shell_probe",
            "mod ime_guest_lifecycle_tests",
        ):
            self.assertIn(marker, BRIDGE)
        self.assertIn(
            "grep -Fqx 'MELANIA_NATIVE_QEMU_IME_THREE_PEER_DISCONNECTS_VERIFIED=true' \"$ime_server_log\"",
            GUEST,
        )
        self.assertIn(
            "fatal 'real_IME_three_authenticated_socket_lifecycles_not_verified'",
            GUEST,
        )
        # The guest still requires independently observed real protocol events:
        # no success based only on the server lifecycle and no bypass of UTF-8.
        self.assertLess(
            GUEST.index("MELANIA_NATIVE_QEMU_IME_THREE_PEER_DISCONNECTS_VERIFIED=true"),
            GUEST.index("'MELANIA_QEMU_REAL_IME_UTF8_COMMIT_SENT=true'"),
        )
        self.assertIn("MELANIA_QEMU_REAL_EDITOR_IME_UTF8_ROUNDTRIP=true", GUEST)
        self.assertIn("MELANIA_QEMU_REAL_IM_V2_TO_TI_V3_UTF8_WIRE_ROUNDTRIP_PASS=true", GUEST)

if __name__ == "__main__":
    unittest.main()
