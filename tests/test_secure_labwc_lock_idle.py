"""Secure lock is a real session protocol, not an attractive fake overlay."""
from pathlib import Path
import subprocess
import unittest
import os
ROOT=Path(__file__).resolve().parents[1]
CTL=(ROOT/"melania-services/src/sessionctl.rs").read_text()
ACTIONS=(ROOT/"melania-panel/src/session_actions.rs").read_text()
LAUNCH=(ROOT/"scripts/melania-labwc-session").read_text()
SUP=(ROOT/"scripts/melania-labwc-supervisor").read_text()
XML=(ROOT/"integration/labwc/rc.xml").read_text()
CARGO=(ROOT/"melania-services/Cargo.toml").read_text()
CI=(ROOT/".github/workflows/linux-verify.yml").read_text()

class SecureLabwcSession(unittest.TestCase):
    def test_rust_lock_is_real_pam_wayland_not_overlay(self):
        self.assertIn('name = "melania-sessionctl"',CARGO)
        self.assertIn('Command::new("swaylock").args(["-f","-c","182D47"])',CTL)
        self.assertIn('Command::new("swayidle").args([',CTL)
        self.assertIn('"before-sleep", "melania-sessionctl lock"',CTL)
        self.assertIn('"timeout", "300", "melania-sessionctl lock"',CTL)
        self.assertIn('"lock", "melania-sessionctl lock"',CTL)
        self.assertIn('MELANIA_SESSION_LOCK_ACQUIRED=true',CTL)
        self.assertNotIn('gtk::Window',CTL)
        self.assertNotIn('loginctl unlock-session',CTL)
        self.assertIn('MELANIA_EXTERNAL_COMPOSITOR',CTL)
        self.assertIn('MELANIA_NESTED_SESSION',CTL)
        self.assertIn('MELANIA_SMOKE_OPEN_START',CTL)

    def test_start_action_requires_real_session_and_lock_confirmation(self):
        self.assertIn('Command::new("melania-sessionctl").arg("check-lock")',ACTIONS)
        self.assertIn('Command::new("melania-sessionctl").arg("lock")',ACTIONS)
        self.assertIn('availability(action)?;',ACTIONS)
        self.assertIn('ControlCommand::Quit',ACTIONS)
        self.assertNotIn('Blocca schermo non ancora implementato nel compositore Melania.',ACTIONS)

    def test_full_login_fails_closed_and_nested_never_starts_idle(self):
        self.assertIn('"$BIN/melania-sessionctl" check-tools',LAUNCH)
        self.assertIn('MELANIA_LABWC_BLOCKED',LAUNCH)
        self.assertIn('if [[ "${MELANIA_NESTED_SESSION:-0}" != 1 ]]; then',SUP)
        self.assertIn('"$BIN/melania-sessionctl" check-lock',SUP)
        self.assertIn('"$BIN/melania-sessionctl" idle &',SUP)
        self.assertIn('MELANIA_LABWC_FATAL=secure_lock_unavailable',SUP)
        self.assertIn('! kill -0 "$idle"',SUP)
        self.assertIn('MELANIA_LABWC_IDLE_GUARD_STARTED=true',SUP)
        self.assertIn('MELANIA_NESTED_SESSION=1',LAUNCH)
        self.assertIn('<keybind key="W-l">',XML)
        self.assertIn('<command>melania-sessionctl lock</command>',XML)
        self.assertIn('cargo build -p melania-compositor -p melania-control -p melania-services',CI)
        self.assertIn('target/debug/melania-sessionctl',CI)

    def test_unchanged_native_login_policy_still_protects_host(self):
        self.assertNotIn('sudo ',SUP)
        self.assertNotIn('sudo ',CTL)
        self.assertNotIn('pkill',SUP)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS',ACTIONS)
        self.assertIn('MELANIA_NESTED_SESSION',ACTIONS)

    def test_session_script_passes_shell_syntax(self):
        for script in ["scripts/melania-labwc-session","scripts/melania-labwc-supervisor"]:
            p=subprocess.run(["bash","-n",str(ROOT/script)],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)

if __name__=="__main__": unittest.main()
