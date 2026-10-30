#!/usr/bin/env python3
"""Offline audit: manifest consistency, local paths and runtime availability.
This does NOT compile Rust or validate third-party GTK/Smithay APIs.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
GROUPS = {
    "core": ["melania-core", "melania-shell-model", "melania-dnd", "melania-transfer", "melania-control", "melania-config", "melania-services", "melania-filectl", "melania-files-core"],
    "gtk": ["melania-files", "melania-settings", "melania-preview", "melania-panel"],
    "compositor": ["melania-compositor"],
}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path, help="Write a machine-readable report")
    args = ap.parse_args()
    problems = []
    checked = []
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    declared = manifest["workspace"]["members"]
    paths = {item: ROOT / item for item in declared}
    for name in declared:
        p = paths[name]
        toml = p / "Cargo.toml"
        if not toml.is_file():
            problems.append(f"Missing manifest: {toml}")
            continue
        data = tomllib.loads(toml.read_text())
        checked.append(name)
        if data.get("package", {}).get("name") != name:
            problems.append(f"Package name mismatch: {name}")
        for dep_name, dep in data.get("dependencies", {}).items():
            if isinstance(dep, dict) and "path" in dep:
                local = (p / dep["path"]).resolve()
                if not local.is_relative_to(ROOT.resolve()) or not (local / "Cargo.toml").is_file():
                    problems.append(f"Invalid local dependency in {name}: {dep_name} -> {local}")
        rsfiles = list((p / "src").glob("*.rs"))
        if not rsfiles:
            problems.append(f"No Rust source in {name}")
    for group, names in GROUPS.items():
        for name in names:
            if name not in declared:
                problems.append(f"Missing from workspace ({group}): {name}")
    required = [
        "branding/icons/melania-symbol.svg", "melania-panel/assets/panel.css",
        "melania-files/assets/style.css", "melania-settings/assets/style.css",
        "docs/UI-SPEC-APPROVATA.md", "scripts/run-melania-v09.sh"
    ]
    for rel in required:
        if not (ROOT / rel).is_file():
            problems.append(f"Missing essential asset: {rel}")
    syms = {p.name: bool(shutil.which(p.name)) for p in [Path(s) for s in ["cargo", "rustc", "pkg-config", "Xvfb", "gcc", "xvfb-run"]]}
    native = {}
    for name in ("gtk4", "gtk4-layer-shell", "wayland-client", "xkbcommon", "egl", "glesv2"):
        if not syms["pkg-config"]:
            native[name] = None
        else:
            cmd = subprocess.run(["pkg-config", "--modversion", name], capture_output=True, text=True)
            native[name] = cmd.stdout.strip() if cmd.returncode == 0 else None
    data = {
        "scope": "offline/static checks only; does not compile Rust",
        "members": checked,
        "member_count": len(checked),
        "groups": GROUPS,
        "executables": syms,
        "native_libraries": native,
        "errors": problems,
        "static_audit": "PASS" if not problems else "FAIL",
        "rust_build": "NOT_RUN" if not syms["cargo"] else "NOT_RUN_BY_THIS_SCRIPT",
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"Workspace: {len(checked)}/{len(declared)} crates found. Static audit: {data['static_audit']}")
    print("Toolchain:", ", ".join(k + "=" + ("yes" if v else "no") for k, v in syms.items()))
    print("Native libraries:", ", ".join(k + "=" + (v or "unavailable") for k, v in native.items()))
    for item in problems:
        print("ERROR:", item, file=sys.stderr)
    return 1 if problems else 0

if __name__ == '__main__':
    sys.exit(main())
