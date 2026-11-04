#!/usr/bin/env python3
"""Stage a reversible Melania labwc session entry; NEVER install it.

Login managers inspect /usr/share/wayland-sessions. This tool intentionally
writes only into the repo's ignored target/ staging directory. Installing
the desktop entry is a separate, explicit administrator action.
"""
from __future__ import annotations
import argparse
import os
import shutil
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
STAGE = ROOT / "target/melania-labwc-session"
SAFE_EXEC = re.compile(r"^/[A-Za-z0-9_./+\-]+$")

def checked(path: Path) -> str:
    value = str(path.resolve())
    if not SAFE_EXEC.fullmatch(value):
        raise ValueError("Percorso non compatibile con Desktop Entry Exec: " + value)
    return value

def entries(bin_dir: Path) -> dict[Path, str]:
    launcher = checked(ROOT / "scripts/melania-labwc-session")
    exe = {n: checked(bin_dir / n) for n in ("melania-files","melania-settings")}
    result = {
        STAGE / "wayland-sessions/org.melaniadesktop.Labwc.desktop":
            ("[Desktop Entry]\nType=Application\nVersion=1.0\n"
             "Name=Melania Desktop (Labwc)\n"
             "Comment=Independent Wayland session with Melania shell and labwc backend\n"
             f"Exec={launcher} --run\nTryExec=/usr/bin/labwc\n"
             "DesktopNames=Melania\nX-Melania-Backend=labwc\n"
             "X-Melania-RustCompositorReady=false\n"),
        STAGE / "share/applications/org.melaniadesktop.Files.desktop":
            ("[Desktop Entry]\nType=Application\n"
             "Name=Melania Files\nGenericName=File Manager\n"
             f"Exec={exe['melania-files']}\nTryExec={exe['melania-files']}\n"
             "Icon=system-file-manager\nCategories=System;FileManager;\n"
             "StartupNotify=true\n"),
        STAGE / "share/applications/org.melaniadesktop.Settings.desktop":
            ("[Desktop Entry]\nType=Application\n"
             "Name=Impostazioni Melania\nGenericName=Desktop Settings\n"
             f"Exec={exe['melania-settings']}\nTryExec={exe['melania-settings']}\n"
             "Icon=preferences-system\nCategories=Settings;\nStartupNotify=true\n"),
        STAGE / "share/xdg-desktop-portal/melania-portals.conf":
            ("[preferred]\n"
             "default=gtk\n"
             "org.freedesktop.impl.portal.Screenshot=wlr\n"
             "org.freedesktop.impl.portal.ScreenCast=wlr\n"),
    }
    return result

def stage(bin_dir: Path) -> list[Path]:
    if not bin_dir.is_absolute():
        raise ValueError("--bin deve essere un percorso assoluto")
    result = entries(bin_dir)
    for path, content in result.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        path.chmod(0o644)
    # Linux Mint 22 / Ubuntu 24.04 does not package GTK4 layer-shell.
    # A user-provided, privately compiled runtime can travel with Melania.
    # Never copy to /usr/lib, replace GTK itself, or modify system loaders.
    private = os.environ.get("MELANIA_LAYER_LIB_DIR")
    if private:
        origin = Path(private) / "libgtk4-layer-shell.so.0"
        if not origin.is_file():
            raise ValueError("Libreria GTK4 layer-shell privata mancante")
        folder = STAGE / "lib"
        folder.mkdir(parents=True, exist_ok=True)
        destination = folder / "libgtk4-layer-shell.so.0"
        if destination.is_symlink():
            raise ValueError("Collegamento non sicuro nella directory privata")
        if origin.resolve() != destination.resolve():
            shutil.copyfile(origin, destination)
        destination.chmod(0o600)
    return list(result)

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--bin", required=True, type=Path)
    args = p.parse_args()
    files = stage(args.bin)
    for item in files:
        print("MELANIA_LABWC_STAGED=" + str(item))
    print("MELANIA_LABWC_HOST_CHANGED=false")
    print("Per aggiungere la sessione a LightDM SOLO dopo la prova annidata:")
    print("sudo install -Dm644 " + str(files[0]) +
          " /usr/share/wayland-sessions/org.melaniadesktop.Labwc.desktop")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
