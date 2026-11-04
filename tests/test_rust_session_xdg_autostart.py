"""XDG autostart must be real and isolated: no nested preview host applications."""
from pathlib import Path
import subprocess
import unittest
ROOT=Path(__file__).resolve().parents[1]
SRC=(ROOT/"melania-settings/src/autostart_main.rs").read_text()
CARGO=(ROOT/"melania-settings/Cargo.toml").read_text()
SUP=(ROOT/"scripts/melania-labwc-supervisor").read_text()
LAUNCH=(ROOT/"scripts/melania-labwc-session").read_text()
CI=(ROOT/".github/workflows/linux-verify.yml").read_text()
class XdgAutostartContracts(unittest.TestCase):
    def test_real_gio_launch_not_shell_or_exec_splitting(self):
        self.assertIn('name = "melania-autostart"',CARGO)
        self.assertIn('DesktopAppInfo::from_filename(&path)',SRC)
        self.assertIn('use gio_unix::DesktopAppInfo;',SRC)
        self.assertIn('gio-unix = "0.22"',CARGO)
        self.assertIn('info.launch(&[],None::<&gio::AppLaunchContext>)',SRC)
        self.assertIn('fn should_launch(source:&str,desktops:&[&str])->bool',SRC)
        for item in ('Hidden','OnlyShowIn','NotShowIn','X-GNOME-Autostart-enabled','Type','Exec'):
            self.assertIn(item,SRC)
        for phrase in ("sh -c","bash -c","Command::new(","std::process::Command"):
            self.assertNotIn(phrase,SRC)
    def test_system_override_precedence_and_explicit_independent_session(self):
        self.assertIn('system.reverse();',SRC)
        self.assertIn('system.push(dir.join("autostart"))',SRC)
        self.assertIn('files.insert(name,item.path());',SRC)
        self.assertIn('MELANIA_NESTED_SESSION',SRC)
        self.assertIn('MELANIA_SMOKE_OPEN_START',SRC)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS',SRC)
        self.assertIn('MELANIA_EXTERNAL_COMPOSITOR',SRC)
    def test_supervisor_runs_autostart_once_only_in_full_login(self):
        self.assertIn('if [[ "${MELANIA_NESTED_SESSION:-0}" != 1 ]]; then',SUP)
        self.assertIn('"$BIN/melania-autostart" &',SUP)
        self.assertIn('MELANIA_LABWC_XDG_AUTOSTART_DISPATCHED=true',SUP)
        self.assertIn('melania-autostart melania-inputctl; do',LAUNCH)
        self.assertIn('cargo test -p melania-settings --bin melania-autostart',CI)
        self.assertNotIn('sudo ',SUP)
        for path in ("scripts/melania-labwc-session","scripts/melania-labwc-supervisor"):
            result=subprocess.run(["bash","-n",str(ROOT/path)],capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stderr)
if __name__=="__main__":unittest.main()
