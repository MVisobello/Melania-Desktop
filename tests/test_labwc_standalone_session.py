"""Standalone labwc-backed Melania session: launcher, protocol and safety tests.

No compositor, seat device, display manager or /usr/share file is touched.
"""
import importlib.util
import os
from pathlib import Path
import socket
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parents[1]
SESSION=(ROOT/"scripts/melania-labwc-session").read_text()
SUPERVISOR=(ROOT/"scripts/melania-labwc-supervisor").read_text()
HOTKEY=(ROOT/"scripts/melania-labwc-hotkey").read_text()

def load(path, name):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    mod=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

bridge=load("scripts/melania-labwc-control.py","labwc_control")
stage=load("scripts/stage-melania-labwc-session.py","labwc_stage")

class StubRunner:
    def __init__(self, listing):
        self.listing=listing
        self.calls=[]
    def __call__(self,args,**kw):
        self.calls.append(args)
        if args[:3]==["wlrctl","toplevel","list"]:
            return SimpleNamespace(returncode=0,stdout=self.listing)
        return SimpleNamespace(returncode=0,stdout="")

class LabwcStandaloneSession(unittest.TestCase):
    def test_real_window_listing_uses_exact_melania_ipc_framing(self):
        stub=StubRunner("org.melaniadesktop.Files: Documenti\nfirefox: Home: Firefox\n")
        server=bridge.Bridge(stub)
        reply=server.dispatch(b"LIST\n").decode()
        self.assertTrue(reply.startswith("OK\nWINDOW\t"))
        self.assertTrue(reply.endswith("END\n"))
        self.assertIn("Documenti\torg.melaniadesktop.Files\n",reply)
        self.assertIn("Home%3A Firefox\tfirefox\n",reply)
        self.assertIn("\t0\t0\t",reply)
        self.assertNotIn("os.system",SESSION+SUPERVISOR)
        self.assertEqual(server.dispatch(b"STATUS\n"),b"OK\nWORKSPACE\t1\nEND\n")
        self.assertEqual(server.dispatch(b"WORKSPACE 1\n"),b"OK\nEND\n")

    def test_window_actions_recheck_real_compositor_toplevel(self):
        stub=StubRunner("firefox: Scheda privata\n")
        ipc=bridge.Bridge(stub)
        rows=ipc.snapshot()
        wid=rows[0][0]
        self.assertEqual(ipc.dispatch(f"ACTIVATE {wid}\n".encode()),b"OK\nEND\n")
        self.assertEqual(stub.calls[-1],["wlrctl","toplevel","focus",
                                         "app_id:firefox","title:Scheda privata"])
        self.assertEqual(ipc.dispatch(f"MINIMIZE {wid}\n".encode()),b"OK\nEND\n")
        self.assertEqual(stub.calls[-1][2],"minimize")
        self.assertTrue(ipc.dispatch(b"PREVIEW 1\n").startswith(b"ERR\t"))

    def test_ambiguous_window_titles_and_ipc_injection_fail_closed(self):
        stub=StubRunner("firefox: Same\nfirefox: Same\n")
        ipc=bridge.Bridge(stub)
        wid=ipc.snapshot()[0][0]
        self.assertTrue(ipc.dispatch(f"CLOSE {wid}\n".encode()).startswith(b"ERR\t"))
        self.assertTrue(ipc.dispatch(b"CLOSE 1\nQUIT\n").startswith(b"ERR\t"))
        self.assertTrue(ipc.dispatch(b"QUIT").startswith(b"ERR\t"))
        self.assertTrue(ipc.dispatch(b"ACTIVATE -1\n").startswith(b"ERR\t"))
        with patch.dict(os.environ,{"LABWC_PID":"invalid"}):
            self.assertTrue(ipc.dispatch(b"QUIT\n").startswith(b"ERR\t"))
        self.assertEqual(len(stub.calls),2)

    def test_unicode_escape_matches_rust_wire_protocol(self):
        self.assertEqual(bridge.escape("Caffè ☕ 100%"),
                         "Caff%C3%A8 %E2%98%95 100%25")
        self.assertEqual(bridge.parse_list("nonsense\nfirefox: Test\n"),
                         [("firefox","Test")])
        with tempfile.TemporaryDirectory() as folder:
            private=Path(folder)
            self.assertTrue(bridge.is_private_runtime(private))
            private.chmod(0o755)
            self.assertFalse(bridge.is_private_runtime(private))

    def test_real_private_unix_socket_accepts_only_bounded_messages(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/"ipc.sock"
            srv=bridge.Server(str(path),bridge.Handler)
            srv.bridge=bridge.Bridge(StubRunner("firefox: Start\n"))
            t=threading.Thread(target=srv.serve_forever,daemon=True)
            t.start()
            try:
                with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as client:
                    client.connect(str(path))
                    client.sendall(b"LIST\n")
                    client.shutdown(socket.SHUT_WR)
                    data=client.recv(4096)
                self.assertIn(b"firefox",data)
                with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as client:
                    client.connect(str(path))
                    client.sendall(b"QUIT\nJUNK\n")
                    client.shutdown(socket.SHUT_WR)
                    data=client.recv(4096)
                self.assertTrue(data.startswith(b"ERR\t"))
            finally:
                srv.shutdown();srv.server_close();t.join(timeout=2)

    def test_stage_only_writes_inside_given_temporary_folder(self):
        with tempfile.TemporaryDirectory() as temp:
            origin=stage.STAGE
            stage.STAGE=Path(temp)/"staging"
            try:
                created=stage.stage(Path(temp)/"bin")
                self.assertEqual(len(created),4)
                portal=created[3].read_text()
                self.assertIn("[preferred]\n",portal)
                self.assertIn("default=gtk\n",portal)
                self.assertIn("org.freedesktop.impl.portal.ScreenCast=wlr",portal)
                self.assertIn("org.freedesktop.impl.portal.Screenshot=wlr",portal)
                entry=created[0].read_text()
                self.assertIn("Name=Melania Desktop (Labwc)",entry)
                self.assertIn(" --run\n",entry)
                self.assertIn("DesktopNames=Melania",entry)
                self.assertNotIn("Exec=labwc",entry)
                self.assertNotIn("/usr/share/wayland-sessions",str(created[0]))
                self.assertIn("Melania Files",created[1].read_text())
                self.assertIn("Impostazioni Melania",created[2].read_text())
            finally:
                stage.STAGE=origin

    def test_no_automatic_host_install_or_cinnamon_replacement(self):
        self.assertIn("--nested-test",SESSION)
        self.assertIn("--run",SESSION)
        self.assertIn("--stage",SESSION)
        self.assertIn("runtime_check()",SESSION)
        self.assertIn('XDG_CURRENT_DESKTOP="Melania:labwc"',SESSION)
        self.assertIn('exec labwc -C "$ROOT/integration/labwc"',SESSION)
        self.assertNotIn("sudo install",SESSION)
        self.assertNotIn("apt install",SUPERVISOR)
        self.assertIn('trap cleanup EXIT',SUPERVISOR)
        self.assertIn('kill "$pid"',SUPERVISOR)
        self.assertIn("TOGGLE_START",HOTKEY+Path(ROOT/"scripts/melania-labwc-control.py").read_text())

    def test_labwc_shortcuts_and_isolated_config_are_well_formed(self):
        xml=ET.parse(ROOT/"integration/labwc/rc.xml").getroot()
        self.assertEqual(xml.tag,"labwc_config")
        shortcuts={k.attrib["key"]:k for k in xml.findall(".//keybind")}
        for name in ("W-space","W-e","W-i","W-Return","A-F4","A-Tab"):
            self.assertIn(name,shortcuts)
        self.assertEqual(shortcuts["W-space"].find(".//command").text,
                         "melania-labwc-hotkey")
        self.assertIn('MELANIA_NESTED_SESSION=1',SESSION)
        self.assertIn("MELANIA_ALLOW_HOST_POWER_ACTIONS=1",SESSION)
        self.assertIn('MELANIA_CONTROL_SOCKET="$XDG_RUNTIME_DIR/melania-labwc-$$.sock"',SESSION)

if __name__=="__main__":
    unittest.main()
