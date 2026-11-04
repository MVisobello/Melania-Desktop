#!/usr/bin/env python3
"""Private Melania taskbar IPC adapter for an independent labwc session.

The existing Rust panel speaks the Melania control protocol. labwc exposes
wlr-foreign-toplevel instead; wlrctl supplies the actual Wayland window list
and the focus/minimize/close commands. No X11 wmctrl or host-wide IPC is used.

This transitional adapter cannot yet provide pixel previews or full workspace
state. Duplicate app-id/title pairs are listed but their actions fail closed
because the wlrctl selector cannot distinguish them.
"""
from __future__ import annotations

import argparse
from collections import Counter
import os
from pathlib import Path
import re
import signal
import socket
import socketserver
import stat
import subprocess
import threading
import time

SAFE = frozenset(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~ ")
MAX_REQUEST = 64
MAX_RESPONSE = 128 * 1024
MAX_WINDOWS = 80

def escape(value: str) -> str:
    return "".join(chr(b) if b in SAFE else "%{:02X}".format(b)
                   for b in value.encode("utf-8"))

def error(message: str) -> bytes:
    return ("ERR\t" + escape(message) + "\nEND\n").encode("ascii")

def parse_list(output: str) -> list[tuple[str, str]]:
    """wlrctl toplevel list emits one 'app_id: title' per Wayland toplevel."""
    windows = []
    for line in output.splitlines():
        if ": " not in line:
            continue
        app, title = line.split(": ", 1)
        if not app or len(app) > 120 or len(title) > 700:
            continue
        if any(c in app + title for c in "\r\n\0"):
            continue
        windows.append((app, title))
        if len(windows) >= MAX_WINDOWS:
            break
    return windows

def is_private_runtime(path: Path) -> bool:
    try:
        d = path.resolve(strict=True)
        m = d.stat()
    except OSError:
        return False
    return (d.is_dir() and m.st_uid == os.getuid()
            and stat.S_IMODE(m.st_mode) & 0o077 == 0)

class Bridge:
    def __init__(self, command_runner=None):
        self.run = command_runner or subprocess.run
        self.lock = threading.Lock()
        self.ids: dict[tuple[str, str, int], int] = {}
        self.next_id = 1
        self.cached_at = 0.0
        self.rows: list[tuple[int, str, str]] = []

    def snapshot(self, refresh: bool = False) -> list[tuple[int, str, str]]:
        with self.lock:
            if not refresh and time.monotonic() - self.cached_at < 0.25:
                return list(self.rows)
            result = self.run(["wlrctl", "toplevel", "list"],
                              capture_output=True, text=True, timeout=0.22,
                              check=False)
            if result.returncode:
                raise RuntimeError("Impossibile leggere le finestre Wayland di labwc")
            counts: Counter[tuple[str, str]] = Counter()
            current = []
            for app, title in parse_list(result.stdout):
                counts[(app, title)] += 1
                identity = (app, title, counts[(app, title)])
                if identity not in self.ids:
                    self.ids[identity] = self.next_id
                    self.next_id += 1
                current.append((self.ids[identity], app, title))
            self.ids = {key: val for key, val in self.ids.items()
                        if (key[0], key[1], key[2]) in {
                            (app, title, ordinal)
                            for ordinal, (app, title) in enumerate_with_duplicates(current)
                        }}
            self.rows = current
            self.cached_at = time.monotonic()
            return list(current)

    def dispatch(self, raw: bytes) -> bytes:
        if not raw.endswith(b"\n") or len(raw) > MAX_REQUEST or b"\n" in raw[:-1]:
            return error("Comando IPC incompleto o non valido")
        try:
            command = raw.decode("ascii").strip("\r\n")
        except UnicodeDecodeError:
            return error("Comando IPC non ASCII")
        if command == "STATUS":
            return b"OK\nWORKSPACE\t1\nEND\n"
        if command == "WORKSPACE 1":
            return b"OK\nEND\n"
        if command == "LIST":
            try:
                rows = self.snapshot()
            except (OSError, subprocess.TimeoutExpired, RuntimeError):
                return error("Elenco finestre Wayland non disponibile")
            result = ["OK\n"]
            for wid, app, title in rows:
                result.append(f"WINDOW\t{wid}\t0\t0\t{escape(title)}\t{escape(app)}\n")
            result.append("END\n")
            data = "".join(result).encode("ascii")
            return data if len(data) <= MAX_RESPONSE else error("Troppe finestre")
        if command == "QUIT":
            return self.exit_session()
        matched = re.fullmatch(r"(ACTIVATE|MINIMIZE|CLOSE|PREVIEW) ([1-9][0-9]*)", command)
        if not matched:
            return error("Operazione non supportata in labwc")
        action, wid = matched.group(1), int(matched.group(2))
        if action == "PREVIEW":
            return error("Miniature non disponibili con labwc")
        try:
            rows = self.snapshot(refresh=True)
        except (OSError, subprocess.TimeoutExpired, RuntimeError):
            return error("Finestre Wayland non disponibili")
        selected = [(app, title) for ident, app, title in rows if ident == wid]
        if len(selected) != 1 or rows.count((wid, *selected[0])) != 1:
            return error("Finestra chiusa o non identificabile")
        app, title = selected[0]
        if sum(1 for _, a, t in rows if a == app and t == title) != 1:
            return error("Finestre identiche: operazione ambigua")
        operation = {"ACTIVATE": "focus", "MINIMIZE": "minimize",
                     "CLOSE": "close"}[action]
        try:
            result = self.run(["wlrctl", "toplevel", operation,
                               "app_id:" + app, "title:" + title],
                              capture_output=True, text=True, timeout=0.25,
                              check=False)
        except (OSError, subprocess.TimeoutExpired):
            return error("Comando Wayland scaduto")
        return b"OK\nEND\n" if result.returncode == 0 else error("Operazione rifiutata da labwc")

    def exit_session(self) -> bytes:
        value = os.environ.get("LABWC_PID", "")
        if not value.isdecimal() or int(value) <= 1:
            return error("PID di sessione labwc mancante")
        pid = int(value)
        try:
            if Path(f"/proc/{pid}").stat().st_uid != os.getuid():
                return error("PID labwc appartenente a un altro utente")
            if Path(f"/proc/{pid}/comm").read_text().strip() != "labwc":
                return error("Il processo da chiudere non è labwc")
        except OSError:
            return error("Compositore labwc già chiuso")
        def terminate():
            time.sleep(0.08)
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        threading.Thread(target=terminate, daemon=True).start()
        return b"OK\nEND\n"

def enumerate_with_duplicates(rows: list[tuple[int, str, str]]):
    counts: Counter[tuple[str, str]] = Counter()
    for _, app, title in rows:
        counts[(app, title)] += 1
        yield (counts[(app, title)], (app, title))

class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(0.35)
        try:
            raw = self.rfile.read(MAX_REQUEST + 1)
            response = self.server.bridge.dispatch(raw)
            self.wfile.write(response)
        except (OSError, TimeoutError):
            pass

class Server(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True
    allow_reuse_address = False

def serve(path: Path) -> None:
    runtime = Path(os.environ.get("XDG_RUNTIME_DIR", ""))
    if not runtime.is_absolute() or not is_private_runtime(runtime):
        raise SystemExit("MELANIA_LABWC_BLOCKED: XDG_RUNTIME_DIR non privato")
    if path.parent != runtime or path.exists() or path.is_symlink():
        raise SystemExit("MELANIA_LABWC_BLOCKED: percorso socket non sicuro")
    old = os.umask(0o077)
    try:
        server = Server(str(path), Handler)
        server.bridge = Bridge()
        os.chmod(path, 0o600)
    finally:
        os.umask(old)
    def interrupt(_signum, _frame):
        raise SystemExit(0)
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    try:
        server.serve_forever(poll_interval=0.1)
    finally:
        server.server_close()
        if path.is_socket():
            path.unlink()

def toggle_start() -> int:
    source = os.environ.get("MELANIA_CONTROL_SOCKET", "")
    if not source:
        return 1
    target = Path(source).with_suffix(".start.sock")
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as client:
            client.sendto(b"TOGGLE_START", str(target))
        return 0
    except OSError:
        return 1

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    options = parser.add_mutually_exclusive_group(required=True)
    options.add_argument("--serve", type=Path)
    options.add_argument("--toggle-start", action="store_true")
    args = parser.parse_args()
    if args.toggle_start:
        return toggle_start()
    serve(args.serve)
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
