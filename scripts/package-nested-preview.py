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
FLUENT_V03_THEMES = ("Melania-Light", "Melania-Dark", "Melania-Light-darkPanel")
FLUENT_V03_PATH = ROOT / "dist/melania-fluent-v03"

# Original app SVGs are shipped rather than embedding icons via system theme:
# the portable archive runs without the source repository.
PREVIEW_APP_ICONS = (
    "melania.svg",
    "org.melaniadesktop.Files.svg",
    "org.melaniadesktop.Settings.svg",
)

# Genuine SVG silhouettes rendered at 22px in the taskbar; unlike symbolic
# masks, these preserve the intended Bluetooth/volume geometry on Mint.
# All strokes are SVG standard shapes with no external resources.
PREVIEW_TRAY_PATHS = {
    "wifi": '<path d="M2.2 8.2c5.6-5.1 14-5.1 19.6 0" />'
            '<path d="M5.6 11.8c3.6-3.3 9.2-3.3 12.8 0" />'
            '<path d="M9 15.5c1.8-1.6 4.2-1.6 6 0" />'
            '<circle cx="12" cy="19" r="1.1" fill="{color}" stroke="none"/>',
    "bluetooth": '<path d="M12 2.2v19.6l6-5.3-12-10.9" />'
                 '<path d="M12 2.2l6 5.3L6 18.4" />',
    "volume": '<path d="M3 9h4l5-4v14l-5-4H3z" />'
              '<path d="M16 9c1.8 1.6 1.8 4.4 0 6" />'
              '<path d="M18.5 6.3c3.2 3.1 3.2 8.3 0 11.4" />',
    "battery": '<rect x="2.2" y="6.8" width="17.8" height="10.4" rx="2.1"/>'
               '<path d="M21 10v4" />'
               '<rect x="5" y="9.4" width="9.4" height="5.2" rx=".8" stroke="none" fill="{color}"/>',
    "notifications": '<path d="M18.4 8.7a6.4 6.4 0 0 0-12.8 0'
                     'c0 6-2.7 6.5-2.7 8.4h18.2c0-1.9-2.7-2.4-2.7-8.4z" />'
                     '<path d="M10 20.1c.9 1.6 3.1 1.6 4 0" />',
}


def build_preview_svg(kind: str, *, light: bool) -> str:
    color = "#24455f" if light else "#edf6ff"
    shapes = PREVIEW_TRAY_PATHS[kind].replace("{color}", color)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" '
            f'viewBox="0 0 24 24" fill="none" stroke="{color}" '
            f'stroke-width="2.05" stroke-linecap="round" '
            f'stroke-linejoin="round">{shapes}</svg>\n')


PREVIEW_ICON_INDEX = """[Icon Theme]
Name=Melania Preview
Comment=Standalone preview icons with native Melania taskbar silhouettes
Inherits=Adwaita,hicolor
Directories=scalable/apps,scalable/status

[scalable/apps]
Size=48
Type=Scalable
MinSize=16
MaxSize=256
Context=Applications

[scalable/status]
Size=24
Type=Scalable
MinSize=16
MaxSize=64
Context=Status
"""


def stage_preview_icons(theme_root: Path, apps: dict[str, Path]) -> None:
    app_dir = theme_root / "scalable/apps"
    status_dir = theme_root / "scalable/status"
    app_dir.mkdir(parents=True)
    status_dir.mkdir(parents=True)
    (theme_root / "index.theme").write_text(PREVIEW_ICON_INDEX, encoding="utf-8")
    for name, source in apps.items():
        shutil.copyfile(source, app_dir / name)
    for kind in PREVIEW_TRAY_PATHS:
        for light in (False, True):
            suffix = "-light" if light else ""
            (status_dir / f"melania-tray-{kind}{suffix}.svg").write_text(
                build_preview_svg(kind, light=light), encoding="utf-8"
            )


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
    preview_apps = {
        name: input_file(ROOT / "branding/Melania/scalable/apps" / name)
        for name in PREVIEW_APP_ICONS
    }
    # A Mint-compatible build may ship ONLY the GTK4 layer-shell bridge built
    # against Ubuntu 24.04. Never copy glibc, GTK4, Mesa or host GPU drivers.
    vendored = None
    if gtk_layer_shell_lib is not None:
        vendored = input_file(gtk_layer_shell_lib)
        with vendored.open("rb") as library_stream:
            magic = library_stream.read(4)
        if magic != b"\x7fELF":
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
    stage_preview_icons(target / "share/icons/Melania-Preview", preview_apps)
    # Include original SVGs and their aliases from a locally staged v0.3 pack.
    # Melania's GTK processes find them through MELANIA_BUNDLE_DATA_DIR.
    has_fluent_v03 = all(
        (FLUENT_V03_PATH / name / "index.theme").is_file()
        for name in FLUENT_V03_THEMES
    )
    if has_fluent_v03:
        for theme_name in FLUENT_V03_THEMES:
            shutil.copytree(
                FLUENT_V03_PATH / theme_name,
                target / "share/icons" / theme_name,
                symlinks=True,
            )
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
        "bundled_icon_theme": "Melania-Light-darkPanel" if has_fluent_v03 else "Melania-Preview",
        "bundled_fluent_v03": has_fluent_v03,
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
                # Allow icon aliases only within the self-contained bundle.
                resolved = path.resolve()
                if not resolved.is_relative_to(target.resolve()) or not resolved.exists():
                    raise ValueError(f"Unsafe icon alias: {path}")
            rel = Path(BUNDLE_NAME) / path.relative_to(target)
            info = tar.gettarinfo(str(path), arcname=rel.as_posix())
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            if path.is_symlink():
                tar.addfile(info)
            elif path.is_file():
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
