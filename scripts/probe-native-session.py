#!/usr/bin/env python3
"""Read-only inspection of prerequisites for a future native Melania session.

This DOES NOT start DRM/KMS, acquire libseat, run a display manager, change
permissions, open hardware nodes, or advertise a working native compositor.
The current compositor unconditionally uses Smithay's winit backend.
"""
from __future__ import annotations

import argparse
import json
import os
import stat
from pathlib import Path
from typing import Mapping


def probe(root: Path = Path("/"), environment: Mapping[str, str] | None = None) -> dict:
    env = os.environ if environment is None else environment
    root = Path(root)
    drm = root / "dev/dri"
    graphics = []
    for node in sorted(drm.glob("card[0-9]*")):
        if not (node.name.startswith("card") and node.name[4:].isdigit()):
            continue
        try:
            metadata = node.stat()
        except OSError:
            continue
        is_device = stat.S_ISCHR(metadata.st_mode)
        graphics.append({
            "name": node.name,
            "character_device": is_device,
            # This is only an advisory filesystem check, never libseat access.
            "filesystem_access": (
                os.access(node, os.R_OK | os.W_OK) if root == Path("/") else None
            ),
        })

    connections = []
    for path in sorted((root / "sys/class/drm").glob("card[0-9]*-*")):
        status_file = path / "status"
        if not status_file.is_file():
            continue
        try:
            status = status_file.read_text(encoding="ascii").strip().lower()
        except (OSError, UnicodeError):
            status = "unknown"
        # Modes being present does NOT demonstrate that KMS works.
        modes_file = path / "modes"
        try:
            modes = [line for line in modes_file.read_text(encoding="ascii").splitlines() if line]
        except (OSError, UnicodeError):
            modes = []
        connections.append({
            "name": path.name,
            "status": status if status in {"connected", "disconnected", "unknown"} else "unknown",
            "advertised_mode_count": len(modes),
        })

    runtime = env.get("XDG_RUNTIME_DIR", "")
    runtime_ok = False
    if runtime:
        candidate = Path(runtime)
        try:
            metadata = candidate.stat()
            runtime_ok = (
                candidate.is_absolute()
                and stat.S_ISDIR(metadata.st_mode)
                and metadata.st_uid == os.getuid()
                and (stat.S_IMODE(metadata.st_mode) & 0o077) == 0
            )
        except OSError:
            pass

    input_nodes = sorted((root / "dev/input").glob("event[0-9]*"))
    # Never open or try to grab input devices during a diagnostic.
    result = {
        "schema_version": 1,
        "backend": "winit-nested",
        "native_backend_compiled": False,
        "native_session_ready": False,
        "status": "blocked",
        "drm_cards": graphics,
        "connectors": connections,
        "input_event_node_count": len(input_nodes),
        "session": {
            "xdg_session_id_present": bool(env.get("XDG_SESSION_ID")),
            "xdg_session_type": env.get("XDG_SESSION_TYPE") or "unknown",
            "xdg_runtime_dir_valid": runtime_ok,
            "already_in_graphical_session": bool(env.get("DISPLAY") or env.get("WAYLAND_DISPLAY")),
        },
        "blocking_requirements": [
            "Implementare e compilare il backend Smithay DRM/KMS/GBM",
            "Gestire seat e VT con libseat/logind e chiusura sicura",
            "Integrare input libinput/udev e aggiornamenti degli output",
            "Verificare login/logout reale in una VM o macchina dedicata",
        ],
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Diagnostica non invasiva della futura sessione nativa")
    parser.add_argument("--json", action="store_true", help="Output leggibile da strumenti di test")
    parser.add_argument(
        "--root", type=Path, default=Path("/"),
        help="Root di fixture sintetiche per test; predefinito: /",
    )
    args = parser.parse_args()
    report = probe(args.root)
    if args.json:
        print(json.dumps(report, indent=2, ensure_ascii=False))
    else:
        print("Melania native preflight — SOLA LETTURA")
        print("Stato: BLOCCATO. Il compositore attuale usa Smithay/Winit annidato.")
        print("Schede DRM visibili:", len(report["drm_cards"]))
        print("Connettori con stato collegato:", sum(
            item["status"] == "connected" for item in report["connectors"]
        ))
        print("Nodi input enumerati:", report["input_event_node_count"])
        print("XDG_RUNTIME_DIR privato:", "sì" if report["session"]["xdg_runtime_dir_valid"] else "no")
        for requirement in report["blocking_requirements"]:
            print(" -", requirement)
        print("Questi dati NON dimostrano la disponibilità di una sessione autonoma.")
    # This exit code is intentional even if DRM devices exist. Do not turn a
    # hardware inventory into a misleading green-light for native launch.
    return 78


if __name__ == "__main__":
    raise SystemExit(main())
