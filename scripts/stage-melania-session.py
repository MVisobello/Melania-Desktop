#!/usr/bin/env python3
"""Stage Melania's existing nested preview as an APPLICATION, never a login session.

This writes only inside the git checkout's ignored Cargo target directory.
There is deliberately no way to install a wayland-sessions/*.desktop entry:
the native Rust backend still has no Wayland client socket or input routing.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
STAGING = ROOT / "target/melania-session-staging/applications"
PREVIEW = ROOT / "scripts/melania-session"
MARKERS = (
    "NATIVE_SESSION_READY=false",
    "visible_kms_commit_allowed(&self) -> bool {false}",
    "actual_kms_submission_enabled(&self)->bool {false}",
)


def readiness() -> dict:
    main = (ROOT / "melania-native/src/main.rs").read_text()
    rollback = (ROOT / "melania-native/src/rollback_audit.rs").read_text()
    frame_gate = (ROOT / "melania-native/src/frame_gate.rs").read_text()
    native_blocked = (
        MARKERS[0] in main and MARKERS[1] in rollback and MARKERS[2] in frame_gate
    )
    return {
        "schema_version": 1,
        "native_ready": False,
        "login_manager_installable": False,
        "nested_preview_available": PREVIEW.is_file(),
        "compiled_native_kms_test_only": (ROOT / "melania-native/src/kms_test.rs").is_file(),
        "blocking_reasons": [
            "No kernel framebuffer and mode-blob ownership for DRM rollback",
            "No verified atomic rollback after visible KMS scanout",
            "No native Wayland wl_seat input forwarding",
            "No standalone Wayland protocol session lifecycle",
        ],
        "fail_closed_markers_present": native_blocked,
    }


def preview_desktop_entry() -> str:
    # Desktop Entry Exec syntax is not a shell: do not interpolate whitespace,
    # quoting, '%' field codes or untrusted paths into the launcher.
    path = str(PREVIEW)
    if not re.fullmatch(r"[/a-zA-Z0-9_.+\-]+", path):
        raise ValueError("checkout path cannot safely be embedded in Desktop Entry Exec")
    if not PREVIEW.is_file():
        raise FileNotFoundError(PREVIEW)
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Version=1.0\n"
        "Name=Melania Desktop (Nested Preview)\n"
        "GenericName=Experimental nested Wayland desktop\n"
        "Comment=Starts only inside an existing graphical session; not a login session\n"
        f"Exec=/usr/bin/env bash {path} --nested\n"
        "TryExec=/usr/bin/env\n"
        "Icon=melania\n"
        "Terminal=false\n"
        "Categories=System;\n"
        "NoDisplay=false\n"
        "StartupNotify=false\n"
        "X-Melania-NativeReady=false\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--status", action="store_true", help="Report native readiness without writes")
    parser.add_argument("--stage-preview", action="store_true",
                        help="Write an APPLICATION launcher under target/ only")
    parser.add_argument("--stage-native", action="store_true",
                        help="Always blocked until safe native Wayland + DRM are implemented")
    args = parser.parse_args()
    if sum((args.status, args.stage_preview, args.stage_native)) != 1:
        parser.error("choose exactly one mode")
    if args.status:
        print(json.dumps(readiness(), indent=2))
        return 0
    if args.stage_native:
        print("NATIVE_LOGIN_SESSION_BLOCKED: not safe for a display manager", flush=True)
        print(json.dumps(readiness(), indent=2))
        return 78
    if not readiness()["fail_closed_markers_present"]:
        print("NESTED_PREVIEW_BLOCKED: safety markers unexpectedly changed", flush=True)
        return 78
    # Only a nested desktop APPLICATION entry. No /usr/share/wayland-sessions,
    # ~/.local/share/wayland-sessions or host system path is ever written.
    content = preview_desktop_entry()
    STAGING.mkdir(parents=True, exist_ok=True)
    output = STAGING / "org.melaniadesktop.NestedPreview.desktop"
    output.write_text(content, encoding="utf-8")
    os.chmod(output, 0o644)
    print(f"MELANIA_NESTED_PREVIEW_STAGED={output}")
    print("MELANIA_NATIVE_LOGIN_SESSION_READY=false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
