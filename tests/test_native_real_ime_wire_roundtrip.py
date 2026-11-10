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
    def test_preedit_and_surrounding_deletion_use_real_utf8_wayland_events(self):
        for marker in (
            'const PREEDIT: &str = "città";',
            'zwp_input_method_v2::Event::SurroundingText',
            't == "prova" && *c == 5 && *a == 5',
            'method.delete_surrounding_text(5, 0)',
            'method.set_preedit_string(PREEDIT.into(), PREEDIT.len() as i32',
            'zwp_text_input_v3::Event::PreeditString',
            'zwp_text_input_v3::Event::DeleteSurroundingText',
            'state.preedit_received.as_ref()',
            'state.deletion_received != Some((5,0))',
            's.payload_done',
            'MELANIA_QEMU_REAL_EDITOR_IME_PREEDIT_DELETE_ROUNDTRIP=true',
            'MELANIA_QEMU_REAL_EDITOR_IME_ATOMIC_DONE=true',
        ):
            self.assertIn(marker,RUST)
        for marker in (
            'MELANIA_QEMU_REAL_IME_SURROUNDING_TEXT_RECEIVED=true',
            'MELANIA_QEMU_REAL_EDITOR_IME_PREEDIT_DELETE_ROUNDTRIP=true',
            'MELANIA_QEMU_REAL_EDITOR_IME_ATOMIC_DONE=true',
            'MELANIA_QEMU_REAL_IME_PREEDIT_AND_DELETE_WIRE_PASS=true',
        ):
            self.assertIn(marker,GUEST)
        self.assertLess(RUST.index('method.delete_surrounding_text(5, 0)'),
                        RUST.index('method.commit(serial)'))
        self.assertLess(RUST.index('method.set_preedit_string('),
                        RUST.index('method.commit(serial)'))

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
            "client.get_credentials(&identity_handle)",
            "credentials.pid as u32==pid",
            'MELANIA_NATIVE_SESSION_READY=false',
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("MELANIA_NATIVE_WAYLAND_RENDER_READY=false",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)

if __name__ == "__main__":
    unittest.main()
