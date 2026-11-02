#!/usr/bin/env python3
"""Stage the original Melania Fluent v0.4 icon archive for builds and previews.

Usage:
    python3 scripts/stage-melania-fluent-v04.py /path/Melania-Fluent-v0.4-Complete.tar.xz

This stages the theme *inside the checkout* under dist/melania-fluent-v04.
It deliberately does not change GNOME/XFCE or the host desktop's icon theme.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
THEMES = ("Melania-Light", "Melania-Dark", "Melania-Light-darkPanel")
TARGET = ROOT / "dist" / "melania-fluent-v04"


def stage(archive: Path, destination: Path = TARGET) -> None:
    if not archive.is_file():
        raise FileNotFoundError(f"Archivio Melania Fluent v0.4 non trovato: {archive}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".melania-fluent-v04-", dir=destination.parent) as tmp:
        staged = Path(tmp)
        with tarfile.open(archive, "r:xz") as tar:
            members = tar.getmembers()
            if not members:
                raise ValueError("Archivio vuoto")
            for member in members:
                path = PurePosixPath(member.name)
                if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] not in THEMES:
                    raise ValueError(f"Percorso non consentito nell'archivio: {member.name}")
                if member.isdir() or member.isfile() or member.issym():
                    if member.issym():
                        link = PurePosixPath(member.linkname)
                        linked_target = (staged / path.parent.as_posix() / member.linkname).resolve()
                        if link.is_absolute() or not linked_target.is_relative_to(staged.resolve()):
                            raise ValueError(f"Collegamento non sicuro: {member.name}")
                else:
                    raise ValueError(f"Tipo di archivio non consentito: {member.name}")
            # Python 3.12+ safe extraction rejects links escaping the stage root.
            tar.extractall(staged, filter="data")
        for theme in THEMES:
            index = staged / theme / "index.theme"
            if not index.is_file():
                raise ValueError(f"Manca index.theme: {theme}")
            source = index.read_text(encoding="utf-8")
            if f"Name={theme}" not in source:
                raise ValueError(f"Nome tema non valido: {theme}")
            if theme == "Melania-Light-darkPanel":
                # v0.4 originally has an empty dark-panel overlay. Inherit our
                # own Light SVGs so this variant actually draws Fluent icons.
                source = source.replace(
                    "Inherits=breeze,elementary,Adwaita,gnome,hicolor",
                    "Inherits=Melania-Light,breeze,elementary,Adwaita,gnome,hicolor",
                )
                index.write_text(source, encoding="utf-8")
        for required in (
            "Melania-Light/places/scalable/folder.svg",
            "Melania-Dark/apps/scalable/utilities-terminal.svg",
        ):
            if not (staged / required).is_file():
                raise ValueError(f"Icona essenziale mancante: {required}")
        # Preserve compatibility with previews compiled before Fluent v0.4.
        legacy = {
            "Melania-Fluent-Light": "Melania-Light",
            "Melania-Fluent-Dark": "Melania-Dark",
            "Melania-Fluent-Light-on-Dark-Panel": "Melania-Light-darkPanel",
        }
        for alias, actual in legacy.items():
            (staged / alias).symlink_to(actual, target_is_directory=True)
        for path in staged.rglob("*"):
            if path.is_symlink() and (
                not path.resolve().is_relative_to(staged.resolve())
                or not path.resolve().exists()
            ):
                raise ValueError(f"Collegamento simbolico non valido: {path}")
        if destination.is_symlink():
            raise ValueError(f"Destination is a symbolic link: {destination}")
        backup = destination.with_name(destination.name + ".old")
        if backup.exists():
            shutil.rmtree(backup)
        if destination.exists():
            destination.rename(backup)
        try:
            staged_out = destination.with_name(destination.name + ".new")
            if staged_out.exists():
                shutil.rmtree(staged_out)
            shutil.move(str(staged), str(staged_out))
            staged_out.rename(destination)
        except Exception:
            if not destination.exists() and backup.exists():
                backup.rename(destination)
            raise
        if backup.exists():
            shutil.rmtree(backup)
    print(f"MELANIA_FLUENT_V04_STAGED={destination}")
    print("I temi sono disponibili per le applicazioni Melania, senza modificare il desktop host.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="Archivio originale Melania Fluent v0.4 .tar.xz")
    args = parser.parse_args()
    stage(args.archive.resolve())
