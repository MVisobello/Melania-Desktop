#!/usr/bin/env python3
"""Package an explicitly non-native Melania Wayland preview from built Linux binaries.

No root privileges, system installation, login-manager entries or host config
writes. The archive is relocatable on compatible Linux systems with the needed
runtime GTK4/Wayland/shared libraries installed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import tarfile

ROOT = Path(__file__).resolve().parents[1]
BINARIES = (
    "melania-compositor",
    "melania-desktop-surface",
    "melania-panel",
    "melania-files",
    "melania-settings",
    "melania-winctl",
    "melania-notifyd",
    "melania-notifyctl",
)
BUNDLE_NAME = "melania-nested-preview"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def input_file(path: Path, *, executable: bool = False) -> Path:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"Missing or unsafe bundle source: {path}")
    if executable and not os.access(path, os.X_OK):
        raise ValueError(f"Binary is not executable: {path}")
    if path.stat().st_size == 0:
        raise ValueError(f"Empty bundle source: {path}")
    return path


def stage(bin_dir: Path, output_dir: Path, commit: str, *,
          gtk_layer_shell_lib: Path | None = None) -> dict:
    if not re.fullmatch(r"[0-9a-fA-F]{40}|unknown", commit):
        raise ValueError("commit must be a complete 40-digit Git SHA or unknown")
    if output_dir.is_symlink() or output_dir.exists():
        raise FileExistsError(f"Refusing to overwrite an existing bundle: {output_dir}")

    # Validate all inputs before creating the output tree.
    sources = {name: input_file(bin_dir / name, executable=True) for name in BINARIES}
    # A Mint-compatible build may ship ONLY the GTK4 layer-shell bridge built
    # against Ubuntu 24.04. Never copy glibc, GTK4, Mesa or host GPU drivers.
    vendored = None
    if gtk_layer_shell_lib is not None:
        vendored = input_file(gtk_layer_shell_lib)
        if vendored.open("rb").read(4) != b"\\x7fELF":
            raise ValueError("Bundled GTK4 layer-shell is not an ELF shared library")
    inputs = {
        "run.sh": input_file(ROOT / "scripts/run-melania-bundle.sh"),
        "share/wallpaper.png": input_file(ROOT / "melania-desktop-surface/assets/wallpaper.png"),
        "README.md": input_file(ROOT / "docs/NESTED-PREVIEW-BUNDLE.md"),
    }

    output_dir.mkdir(parents=True, exist_ok=False)
    target = output_dir / BUNDLE_NAME
    (target / "bin").mkdir(parents=True)
    (target / "share").mkdir()
    if vendored is not None:
        (target / "lib").mkdir()
        destination = target / "lib/libgtk4-layer-shell.so.0"
        shutil.copyfile(vendored, destination)
        destination.chmod(0o644)
    for name, source in sources.items():
        dest = target / "bin" / name
        shutil.copyfile(source, dest)
        dest.chmod(0o755)
    for name, source in inputs.items():
        dest = target / name
        shutil.copyfile(source, dest)
        dest.chmod(0o755 if name == "run.sh" else 0o644)

    files = sorted(p for p in target.rglob("*") if p.is_file())
    manifest = {
        "schema_version": 1,
        "kind": "melania-nested-development-preview",
        "source_commit": commit.lower(),
        "native_session_installable": False,
        "requires_existing_graphical_session": True,
        "requires_compatible_linux_runtime_libraries": True,
        "vendored_gtk4_layer_shell": vendored is not None,
        "vendored_libraries": ["lib/libgtk4-layer-shell.so.0"] if vendored is not None else [],
        "files_sha256": {
            p.relative_to(target).as_posix(): sha256(p) for p in files
        },
    }
    manifest_path = target / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    manifest_path.chmod(0o644)

    archive = output_dir / f"{BUNDLE_NAME}.tar.gz"
    with tarfile.open(archive, mode="w:gz", format=tarfile.PAX_FORMAT) as tar:
        for path in sorted(target.rglob("*")):
            if path.is_symlink():
                raise ValueError(f"Refusing symlink in package: {path}")
            rel = Path(BUNDLE_NAME) / path.relative_to(target)
            info = tar.gettarinfo(str(path), arcname=rel.as_posix())
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            if path.is_file():
                with path.open("rb") as src:
                    tar.addfile(info, src)
            else:
                tar.addfile(info)

    checksum_path = output_dir / "SHA256SUMS"
    checksum_path.write_text(f"{sha256(archive)}  {archive.name}\n", encoding="ascii")
    print(f"MELANIA_NESTED_PREVIEW_ARCHIVE={archive}")
    print(f"MELANIA_NESTED_PREVIEW_SHA256={sha256(archive)}")
    print("MELANIA_NATIVE_LOGIN_SESSION_READY=false")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, default=ROOT / "target/debug")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/melania-preview-release")
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--gtk-layer-shell-lib", type=Path,
                        help="Optional private GTK4 layer-shell ELF, built on the target ABI baseline")
    args = parser.parse_args()
    stage(args.bin_dir, args.output_dir, args.commit,
          gtk_layer_shell_lib=args.gtk_layer_shell_lib)


if __name__ == "__main__":
    main()
