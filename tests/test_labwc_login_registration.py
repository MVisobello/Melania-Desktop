"""Stage101: Melania login manager opt-in and rollback never touch host /usr."""
from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("melania_login_manager",
                                             ROOT / "scripts/manage-melania-labwc-login.py")
login = importlib.util.module_from_spec(spec)
spec.loader.exec_module(login)


class ReversibleLabwcLogin(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="melania-login-test-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.system_root = self.base / "disposable-system"
        self.system_root.mkdir()
        self.config = self.base / "user-config"
        self.config.mkdir(mode=0o700)
        self.checkout = self.base / "my-own-repository"
        (self.checkout / "scripts").mkdir(parents=True)
        fake = self.checkout / "scripts/melania-labwc-session"
        fake.write_text("#!/usr/bin/env bash\n"
                        'printf "MELANIA_LOGIN_DISPATCHED=%s\\n" "$1"\n')
        fake.chmod(0o755)

    def test_system_entry_is_root_owned_generic_and_reversible(self):
        created = login.system_install(self.system_root)
        self.assertEqual(len(created), 2)
        entry = login._system_path(self.system_root, login.ENTRY_PATH)
        wrapper = login._system_path(self.system_root, login.LAUNCH_PATH)
        self.assertEqual(entry.stat().st_mode & 0o777, 0o644)
        self.assertEqual(wrapper.stat().st_mode & 0o777, 0o755)
        self.assertIn(f"Exec={login.LAUNCH_PATH}\n",entry.read_text())
        self.assertIn("X-Melania-RustCompositorReady=false",entry.read_text())
        self.assertNotIn(str(self.checkout),entry.read_text())
        self.assertNotIn(str(ROOT),entry.read_text())
        self.assertEqual(set(login.system_install(self.system_root)),set(created))
        self.assertEqual(set(login.system_uninstall(self.system_root)),set(created))
        self.assertEqual(login.system_uninstall(self.system_root),[])
        self.assertFalse(entry.exists())
        self.assertFalse(wrapper.exists())

    def test_system_install_does_not_replace_unrelated_login(self):
        entry = login._system_path(self.system_root, login.ENTRY_PATH)
        entry.parent.mkdir(parents=True)
        entry.write_text("[Desktop Entry]\nName=Other Desktop\n")
        with self.assertRaisesRegex(ValueError,"not managed"):
            login.system_install(self.system_root)
        self.assertFalse(login._system_path(self.system_root,login.LAUNCH_PATH).exists())
        self.assertIn("Other Desktop",entry.read_text())

    def test_system_uninstall_fails_before_touching_any_foreign_file(self):
        login.system_install(self.system_root)
        entry = login._system_path(self.system_root, login.ENTRY_PATH)
        wrapper = login._system_path(self.system_root, login.LAUNCH_PATH)
        entry.write_text("[Desktop Entry]\nName=Other Desktop\n")
        with self.assertRaisesRegex(ValueError,"not managed"):
            login.system_uninstall(self.system_root)
        self.assertTrue(wrapper.is_file())
        self.assertIn("Other Desktop",entry.read_text())

    def test_system_install_rejects_symlinks_without_overwrite(self):
        outside = self.base/"important"
        outside.write_text("preserve")
        path = login._system_path(self.system_root,login.ENTRY_PATH)
        path.parent.mkdir(parents=True)
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError,"symbolic link"):
            login.system_install(self.system_root)
        self.assertEqual(outside.read_text(),"preserve")
        self.assertFalse(login._system_path(self.system_root,login.LAUNCH_PATH).exists())

    def test_user_registration_is_isolated_private_and_undoable(self):
        path=login.register_user(self.config,self.checkout,verify=False)
        self.assertEqual(path.stat().st_mode&0o777,0o600)
        self.assertEqual(path.parent.stat().st_mode&0o777,0o700)
        self.assertEqual(path.read_text(),
                         f"{login.USER_MAGIC}\n{self.checkout}\n")
        self.assertEqual(login.register_user(self.config,self.checkout,verify=False),path)
        self.assertTrue(login.unregister_user(self.config))
        self.assertFalse(login.unregister_user(self.config))
        self.assertFalse(path.exists())

    @unittest.skipIf(os.geteuid()==0,"real LightDM login scripts are user processes")
    def test_real_generic_wrapper_runs_only_the_opted_in_checkout(self):
        wrapper=ROOT/"scripts/melania-labwc-login-launch"
        env=dict(os.environ,HOME=str(self.base),XDG_CONFIG_HOME=str(self.config))
        blank=subprocess.run(["bash",str(wrapper)],env=env,text=True,
                             capture_output=True,timeout=5)
        self.assertEqual(blank.returncode,78,blank.stderr)
        self.assertIn("not registered",blank.stderr)
        login.register_user(self.config,self.checkout,verify=False)
        accepted=subprocess.run(["bash",str(wrapper)],env=env,text=True,
                                capture_output=True,timeout=5)
        self.assertEqual(accepted.returncode,0,accepted.stderr)
        self.assertIn("MELANIA_LOGIN_DISPATCHED=--run",accepted.stdout)
        path=self.config/"melania"/login.USER_FILE
        path.chmod(0o644)
        exposed=subprocess.run(["bash",str(wrapper)],env=env,text=True,
                               capture_output=True,timeout=5)
        self.assertEqual(exposed.returncode,78,exposed.stderr)
        self.assertIn("not private",exposed.stderr)

    def test_untrusted_registration_and_symlink_are_refused(self):
        folder=self.config/"melania"
        folder.mkdir(mode=0o700)
        path=folder/login.USER_FILE
        path.write_text("other user's login setup\n")
        path.chmod(0o600)
        with self.assertRaisesRegex(ValueError,"not managed"):
            login.register_user(self.config,self.checkout,verify=False)
        path.unlink()
        outside=self.base/"important"
        outside.write_text("keep")
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError,"symlink"):
            login.unregister_user(self.config)
        self.assertEqual(outside.read_text(),"keep")

    def test_syntax_and_explicit_user_opt_in(self):
        subprocess.run(["bash","-n",str(ROOT/"scripts/melania-labwc-login-launch")],
                       check=True,timeout=5)
        launch=(ROOT/"scripts/melania-labwc-login-launch").read_text()
        self.assertIn("MELANIA_LABWC_USER_ROOT_V1",launch)
        self.assertIn("exec /usr/bin/env bash",launch)
        self.assertIn('"$root/scripts/melania-labwc-session" --run',launch)
        self.assertNotIn("sudo",launch)
        self.assertNotIn("eval ",launch)
        entry=login.ENTRY.decode()
        self.assertIn("DesktopNames=Melania",entry)
        self.assertNotIn("X-Melania-RustCompositorReady=true",entry)


if __name__=="__main__":
    unittest.main(verbosity=2)
