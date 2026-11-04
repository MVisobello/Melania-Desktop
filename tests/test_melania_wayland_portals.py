"""Melania-specific XDG portal file must not alter Cinnamon or run host daemons."""
import configparser
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import importlib.util
ROOT=Path(__file__).resolve().parents[1]
script=ROOT/"scripts/stage-melania-labwc-session.py"
spec=importlib.util.spec_from_file_location("melania_portals_stage",script)
mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
LAUNCH=(ROOT/"scripts/melania-labwc-session").read_text()
SUP=(ROOT/"scripts/melania-labwc-supervisor").read_text()

class WaylandPortalContract(unittest.TestCase):
    def test_desktop_specific_ini_staged_in_private_xdg_data_dir(self):
        with tempfile.TemporaryDirectory() as tmp:
            old=mod.STAGE
            mod.STAGE=Path(tmp)/"melania"
            try:
                entries=mod.stage(Path(tmp)/"bin")
                portals=mod.STAGE/"share/xdg-desktop-portal/melania-portals.conf"
                self.assertIn(portals,entries)
                cfg=configparser.ConfigParser()
                cfg.read(portals)
                self.assertEqual(cfg["preferred"]["default"],"gtk")
                self.assertEqual(cfg["preferred"]["org.freedesktop.impl.portal.Screenshot"],"wlr")
                self.assertEqual(cfg["preferred"]["org.freedesktop.impl.portal.ScreenCast"],"wlr")
                self.assertFalse((Path(tmp)/"xdg-desktop-portal/portals.conf").exists())
            finally: mod.STAGE=old

    def test_only_active_melania_session_exports_wayland_to_its_bus(self):
        self.assertIn('XDG_CURRENT_DESKTOP="Melania:labwc"',LAUNCH)
        self.assertIn('XDG_DATA_DIRS="$STAGE/share:',LAUNCH)
        self.assertIn('dbus-update-activation-environment WAYLAND_DISPLAY',SUP)
        self.assertIn('dbus-update-activation-environment --systemd WAYLAND_DISPLAY',SUP)
        self.assertIn('MELANIA_NESTED_SESSION',SUP)
        self.assertNotIn('systemctl --user restart xdg-desktop-portal',SUP)
        self.assertNotIn('sudo ',SUP)

if __name__=="__main__": unittest.main()
