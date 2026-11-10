"""Wayland text-input v3 + input-method v2 VM protocol safety contract.

The Smithay seat forwards focus and IME events, but the privileged IME
global must not be advertised to untrusted Wayland clients or enabled for
production before compositor popup and physical-input verification.
This static audit is NOT a cross-process IME acceptance test.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
ISSUE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class NativeTextInputImeGate(unittest.TestCase):
    def test_text_input_and_ime_are_paired_and_explicitly_vm_only(self):
        for expected in (
            "delegate_text_input_manager!(BridgeState)",
            "delegate_input_method_manager!(BridgeState)",
            "InputMethodHandler for BridgeState",
            "MELANIA_NATIVE_TEXT_INPUT_IME",
            "MELANIA_NATIVE_TRUSTED_IME_PID",
            'as_deref()==Ok("YES_QEMU_VM")',
            'return Err("text input+IME probe may only run in disposable QEMU VM".into())',
            'Some(TextInputManagerState::new::<BridgeState>(&handle))',
            'Some(InputMethodManagerState::new::<BridgeState,_>(&handle,',
            'client.get_credentials(&identity_handle)',
            'credentials.pid as u32==pid',
            "MELANIA_NATIVE_QEMU_TEXT_INPUT_V3_AND_PID_GATED_IME_READY=true",
        ):
            self.assertIn(expected,BRIDGE)
        gate=BRIDGE.index('let ime_probe=std::env::var("MELANIA_NATIVE_TEXT_INPUT_IME")')
        globals_=BRIDGE.index("let _text_input_global=")
        self.assertLess(gate,globals_)
        self.assertLess(BRIDGE.index("let trusted_ime_pid=if ime_probe"),globals_)
        self.assertIn("if trusted_ime_pid.is_some()",BRIDGE)
    def test_candidate_panel_is_not_falsely_declared_rendered(self):
        self.assertIn("MELANIA_NATIVE_IME_POPUP_NOT_RENDERED=true",BRIDGE)
        self.assertIn("fn new_popup(&mut self,_surface:InputMethodPopupSurface)",BRIDGE)
        self.assertNotIn("MELANIA_NATIVE_IME_POPUP_KMS_PRESENTED=true",BRIDGE)
        self.assertIn("MELANIA_NATIVE_WAYLAND_RENDER_READY=false",BRIDGE)
        self.assertIn("NATIVE_SESSION_READY=false",BRIDGE)

if __name__=="__main__":
    unittest.main()
