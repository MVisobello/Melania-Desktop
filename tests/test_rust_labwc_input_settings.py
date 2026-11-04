"""Rust per-user libinput preferences: never modify Cinnamon or host XDG labwc."""
from pathlib import Path
import subprocess
import unittest
import xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[1]
RUST=(ROOT/"melania-services/src/inputctl.rs").read_text()
UI=(ROOT/"melania-settings/src/input_controls.rs").read_text()
LAUNCH=(ROOT/"scripts/melania-labwc-session").read_text()
CARGO=(ROOT/"melania-services/Cargo.toml").read_text()
GNOME=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()
XML=(ROOT/"integration/labwc/rc.xml").read_text()
CI=(ROOT/".github/workflows/linux-verify.yml").read_text()

class MelaniaInputConfig(unittest.TestCase):
    def test_live_rust_input_tool_and_gtk_ui_are_compiled(self):
        self.assertIn('name = "melania-inputctl"',CARGO)
        self.assertIn('mod input_controls;',MAIN)
        self.assertIn('crate::input_controls::append(&devices);',GNOME)
        self.assertIn('Command::new("melania-inputctl").arg("show")',UI)
        self.assertIn('Command::new("melania-inputctl").arg("apply")',UI)
        self.assertIn('Command::new("labwc").arg("--reconfigure")',RUST)
        for element in ('<tap>','<naturalScroll>','<leftHanded>','<pointerSpeed>'):
            self.assertIn(element,RUST)
        self.assertIn('target/debug/melania-inputctl',CI)

    def test_private_config_does_not_reconfigure_cinnamon(self):
        self.assertIn('MELANIA_NESTED_SESSION',RUST)
        self.assertIn('MELANIA_EXTERNAL_COMPOSITOR',RUST)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS',RUST)
        self.assertIn('PathBuf::from(home).join(".config")',RUST)
        self.assertIn('base.join("labwc").join("rc.xml")',RUST)
        self.assertIn('CONFIG_HEADER',RUST)
        self.assertIn('create_new(true).write(true).mode(0o600)',RUST)
        self.assertIn('refuse to overwrite manually edited labwc config',RUST)
        self.assertNotIn('sudo ',RUST)
        self.assertNotIn('~/.config/labwc',LAUNCH)
        self.assertIn('exec labwc -C "$ROOT/integration/labwc"',LAUNCH)
        self.assertIn('exec labwc -C "$MELANIA_LABWC_CONFIG_DIR"',LAUNCH)
        self.assertIn('"$BIN/melania-inputctl" prepare',LAUNCH)

    def test_template_remains_valid_and_shell_script_passes_syntax(self):
        ET.fromstring(XML)
        self.assertIn('<keyboard>',XML)
        for file in ('scripts/melania-labwc-session','scripts/melania-labwc-supervisor'):
            result=subprocess.run(['bash','-n',str(ROOT/file)],capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stderr)

if __name__=='__main__': unittest.main()
