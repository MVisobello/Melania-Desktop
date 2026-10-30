#!/usr/bin/env python3
"""Build original Melania Aero freedesktop themes from icon-name inventories.

The supplied Flat Remix archive is used ONLY for its filesystem naming scheme.
This program never opens, embeds, renders or copies any Flat Remix icon artwork.
It reuses the project's own Melania Aero Classic originals, supplementing
uncovered names with generated original blue-glass vector symbols.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import os
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
CLASSIC = ROOT / "branding" / "Melania-Aero-Classic"
MELANIA = ROOT / "branding" / "Melania"
COVERAGE = ROOT / "branding" / "coverage"
CATEGORIES = {
    "actions": "Actions", "apps": "Applications", "animations": "Animations",
    "categories": "Categories", "devices": "Devices", "emblems": "Emblems",
    "mimetypes": "MimeTypes", "panel": "Status",
    "places": "Places", "status": "Status",
}
FALLBACK = {
    # Only unresolved *place* aliases share an existing folder. Unknown apps
    # get distinctive original glass vector marks, not 7000 identical
    # "applications-other" tiles.
    "places": "folder",
}
PALETTE = ["#2785dc", "#1da6c7", "#537bcd", "#328b86",
           "#a267b4", "#499b60", "#d08243", "#4d7ba8"]

def inventory(variant: str) -> list[str]:
    file = COVERAGE / f"reference-{variant}-paths.txt"
    paths = [line.strip() for line in file.read_text(encoding="utf8").splitlines()
             if line.strip() and not line.startswith("#")]
    if len(paths) != len(set(paths)):
        raise ValueError(f"Duplicate inventory path in {file}")
    for path in paths:
        p = Path(path)
        if p.is_absolute() or ".." in p.parts or not p.parts or p.parts[0] not in CATEGORIES:
            raise ValueError(f"Unsafe path: {path}")
        if p.suffix.lower() != ".svg":
            raise ValueError(f"Not SVG: {path}")
    return paths

def semantic(name: str, category: str) -> str:
    n = name.lower().replace("_", "-")
    if "folder" in n or "directory" in n or "inode-directory" in n:
        return "folder"
    if "trash" in n: return "trash"
    if "home" in n: return "home"
    if "network" in n or "wifi" in n or "wireless" in n: return "network"
    if "bluetooth" in n: return "bluetooth"
    if "battery" in n or "power" in n: return "power"
    if "volume" in n or "audio" in n or "sound" in n: return "audio"
    if "image" in n or "photo" in n or "picture" in n: return "picture"
    if "video" in n or "movie" in n or "film" in n: return "video"
    if "music" in n or "mp3" in n: return "music"
    if "document" in n or "text" in n or "pdf" in n or "office" in n: return "document"
    if "search" in n or "find" in n: return "search"
    if "download" in n or "save" in n: return "download"
    if "upload" in n: return "upload"
    if "settings" in n or "preferences" in n or "configure" in n: return "settings"
    if "edit-copy" in n or "copy" in n: return "copy"
    if "edit-cut" in n or "cut" in n: return "cut"
    if "paste" in n: return "paste"
    if "add" in n or "new" in n: return "plus"
    if "remove" in n or "delete" in n or "close" in n: return "minus"
    if "go-next" in n or "forward" in n or "right" in n: return "next"
    if "go-previous" in n or "back" in n or "left" in n: return "back"
    if "refresh" in n or "reload" in n or "sync" in n: return "refresh"
    if "monitor" in n or "display" in n or "computer" in n: return "computer"
    if "printer" in n or "print" in n: return "printer"
    if "lock" in n or "secure" in n or "shield" in n: return "lock"
    if category == "mimetypes": return "document"
    if category == "devices": return "device"
    if category == "actions": return "action"
    if category == "places": return "folder"
    return "application"

def source_candidates(name: str, category: str):
    base = name.removesuffix("-symbolic")
    for candidate in (name, base):
        yield CLASSIC / "scalable" / category / f"{candidate}.svg"
        yield MELANIA / "scalable" / category / f"{candidate}.svg"
        yield CLASSIC / "128x128" / category / f"{candidate}.png"
        yield MELANIA / "128x128" / category / f"{candidate}.png"
    # Keep the existing Aero visual family rather than guessing foreign art.
    fallback = FALLBACK.get(category)
    if fallback:
        yield CLASSIC / "128x128" / category / f"{fallback}.png"
        yield MELANIA / "scalable" / category / f"{fallback}.svg"

def original_svg(name: str, category: str) -> str | None:
    for p in source_candidates(name, category):
        if not p.is_file():
            continue
        if p.suffix == ".svg":
            return p.read_text(encoding="utf8")
        data = base64.b64encode(p.read_bytes()).decode("ascii")
        # Standalone wrapper; the PNG belongs to Melania Aero Classic.
        return ('<svg xmlns="http://www.w3.org/2000/svg" '
                'viewBox="0 0 128 128" width="128" height="128">'
                '<image width="128" height="128" '
                f'href="data:image/png;base64,{data}"/></svg>')
    return None

# Original Melania glyph vocabulary, deliberately not derived from Flat Remix.
GLYPHS = {
 "folder": '<path d="M11 31Q11 24 18 24H40L47 32H77Q85 32 85 40V69Q85 77 77 77H18Q11 77 11 70Z"/><path d="M13 45H83L79 73H19Z" fill="url(#shine)"/>',
 "home": '<path d="M17 45 48 19 79 45M25 43V77H71V43M42 77V56H56V77"/>',
 "document": '<path d="M25 16H59L73 30V79H25ZM59 16V31H73M34 44H65M34 55H62M34 65H57"/>',
 "picture": '<rect x="17" y="22" width="62" height="54" rx="6"/><circle cx="58" cy="38" r="6"/><path d="m22 66 19-19 12 12 9-9 13 13"/>',
 "video": '<rect x="17" y="24" width="62" height="49" rx="8"/><path d="m39 36 22 13-22 13Z"/>',
 "music": '<path d="M41 65V30L70 24V61M41 35 70 29"/><ellipse cx="32" cy="69" rx="9" ry="5"/><ellipse cx="61" cy="64" rx="9" ry="5"/>',
 "trash": '<path d="M29 32H67L62 76H35ZM24 32H73M39 25H58M43 43V65M54 43V65"/>',
 "network": '<circle cx="48" cy="25" r="9"/><circle cx="25" cy="67" r="9"/><circle cx="71" cy="67" r="9"/><path d="m43 33-14 25m24-25 14 25M34 67h28"/>',
 "bluetooth": '<path d="M45 16V81L65 62 30 34M45 16 65 35 30 65"/>',
 "power": '<path d="M48 18V50M34 27C7 45 28 78 48 78S89 45 62 27"/>',
 "audio": '<path d="M21 42H35L49 29V67L35 54H21ZM57 39Q70 48 57 58M65 29Q86 49 65 69"/>',
 "computer": '<rect x="16" y="23" width="64" height="45" rx="6"/><path d="M48 69V79M35 79H61"/>',
 "printer": '<rect x="26" y="18" width="44" height="25"/><rect x="18" y="38" width="60" height="29" rx="6"/><path d="M30 55H66V78H30Z"/>',
 "settings": '<circle cx="48" cy="48" r="23" stroke-dasharray="11 6"/><circle cx="48" cy="48" r="11"/>',
 "search": '<circle cx="42" cy="41" r="19"/><path d="m56 56 18 18"/>',
 "download": '<path d="M48 19V64m-16-14 16 15 16-15M23 71H73"/>',
 "upload": '<path d="M48 76V31M32 45 48 30l16 15M23 75H73"/>',
 "copy": '<rect x="19" y="21" width="43" height="44" rx="5"/><rect x="33" y="34" width="43" height="44" rx="5"/>',
 "paste": '<rect x="28" y="28" width="43" height="50" rx="6"/><rect x="39" y="22" width="22" height="13" rx="4"/>',
 "cut": '<circle cx="29" cy="67" r="8"/><circle cx="68" cy="67" r="8"/><path d="m31 25 35 35M66 25 31 60"/>',
 "plus": '<path d="M48 23V73M23 48H73"/>',
 "minus": '<path d="M25 48H72"/>',
 "back": '<path d="M61 24 36 48l25 24M37 48H79"/>',
 "next": '<path d="m36 24 25 24-25 24M18 48H61"/>',
 "refresh": '<path d="M73 40A26 26 0 0 0 28 34m0-12v17h17M23 56A26 26 0 0 0 68 63m0 11V57H51"/>',
 "lock": '<rect x="27" y="41" width="42" height="37" rx="6"/><path d="M35 42V32a13 13 0 0 1 26 0v10"/>',
 "device": '<rect x="19" y="25" width="58" height="44" rx="6"/><circle cx="33" cy="48" r="5"/><path d="M47 48H67M47 58H65"/>',
 "action": '<path d="M26 48H70M50 28l21 20-21 20"/>',
 "application": '<rect x="16" y="17" width="64" height="62" rx="13"/><path d="M17 36H79M29 27H32M40 27H43M51 27H54"/>',
}

def generated_svg(name: str, category: str, variant: str) -> str:
    key = semantic(name,category)
    digest = hashlib.sha256(name.encode()).digest()
    tint = PALETTE[digest[0] % len(PALETTE)]
    symbolic = name.endswith("-symbolic") or variant.endswith("-symbolic")
    dark = variant.startswith("dark")
    fg = "#ecf7ff" if dark else "#17416a"
    if symbolic:
        return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">'
                f'<g fill="none" stroke="{fg}" stroke-width="5" '
                f'stroke-linecap="round" stroke-linejoin="round">{GLYPHS[key]}</g></svg>')
    glyph = GLYPHS[key]
    if category == "actions":
        # Compact GTK toolbar glyphs must stay legible at 16/22 px without
        # a square tile behind each action. Different contrast per variant.
        ink = "#edf8ff" if variant == "dark-action" else "#194c7c"
        accent = "#5ac4ff" if variant == "dark-action" else "#1887da"
        return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">'
                f'<defs><linearGradient id="ink" x2="1" y2="1">'
                f'<stop stop-color="{accent}"/><stop offset="1" stop-color="{ink}"/>'
                f'</linearGradient></defs>'
                f'<g fill="none" stroke="url(#ink)" stroke-width="5.5" '
                f'stroke-linecap="round" stroke-linejoin="round">{glyph}</g></svg>')
    # Glass treatment and highlights follow the original Melania Aero palette.
    badge = ""
    if key == "application":
        letters = "".join(c for c in name.split(".")[-1] if c.isalpha())[:2].upper()
        if not letters: letters = "A"
        badge = (f'<text x="48" y="63" font-size="26" text-anchor="middle" '
                 f'font-family="DejaVu Sans,sans-serif" font-weight="bold" '
                 f'fill="#ffffff">{letters}</text>')
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">
<defs><linearGradient id="face" x2=".65" y2="1">
<stop stop-color="#9fddff"/><stop offset=".46" stop-color="{tint}"/>
<stop offset="1" stop-color="#155391"/></linearGradient>
<linearGradient id="shine" x2="0" y2="1">
<stop stop-color="#fff" stop-opacity=".65"/>
<stop offset="1" stop-color="#fff" stop-opacity=".04"/></linearGradient></defs>
<ellipse cx="48" cy="85" rx="34" ry="4" fill="#0b2341" opacity=".16"/>
<rect x="9" y="9" width="78" height="76" rx="18" fill="url(#face)"
 stroke="#154d82" stroke-width="1.8"/>
<path d="M12 37V28Q12 12 30 12H66Q83 12 83 29V39Q47 23 12 37Z"
 fill="url(#shine)" opacity=".67"/>
<g fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round"
 stroke-linejoin="round">{glyph}</g>{badge}</svg>'''

def index_theme(name: str, source_variant: str, directory_paths: list[str]) -> str:
    sections = []
    for sub in sorted(directory_paths):
        context = CATEGORIES[sub.split("/")[0]]
        size = 16
        scal = "symbolic" in sub or "scalable" in sub or sub in ("panel","animations")
        if not scal:
            for part in sub.split("/")[1:]:
                if part.isdigit():
                    size=int(part)
                    break
        lines=[f"[{sub}]",f"Context={context}",f"Size={size}",
               f"Type={'Scalable' if scal else 'Fixed'}"]
        if scal:lines+=["MinSize=16","MaxSize=512"]
        sections.append("\n".join(lines))
    dirs=",".join(sorted(directory_paths))
    return (f"[Icon Theme]\nName={name}\nName[it]={name}\n"
            "Comment=Original Melania Aero glass artwork with extended freedesktop compatibility\n"
            "Inherits=Melania-Aero-Classic,Adwaita,hicolor\n"
            "Example=folder\nFollowsColorScheme=true\n"
            "DesktopDefault=48\nToolbarDefault=22\nPanelDefault=24\n"
            f"Directories={dirs}\n\n"+"\n\n".join(sections)+"\n")

def symlink_relative(destination: Path, target: Path) -> None:
    destination.parent.mkdir(parents=True,exist_ok=True)
    destination.symlink_to(os.path.relpath(target,destination.parent))

def build(dest: Path, dark_paths: list[str], light_paths: list[str]) -> dict:
    dark=dest/"Melania-Dark";light=dest/"Melania-Light"
    mixed=dest/"Melania-Light-on-Dark-Panel"
    dark.mkdir(parents=True,exist_ok=True)
    light.mkdir(parents=True,exist_ok=True)
    mixed.mkdir(parents=True,exist_ok=True)
    paths = sorted(set(dark_paths)|set(light_paths))
    created:dict[tuple[str,str,str],Path]={}
    origins = {"original":0,"generated":0}
    for variant,root,chosen in [("dark",dark,paths),("light",light,paths)]:
        for path in chosen:
            parts=Path(path).parts
            context=parts[0]; name=Path(path).stem
            symbol = name.endswith("-symbolic") or "symbolic" in parts
            # Light windows require dark symbolic glyphs in *all* contexts,
            # while colorful app icons are shared from the original Aero
            # family. Status icons are separate for light panels.
            if variant=="light" and not (
                context in ("actions","panel") or symbol
            ):
                continue
            key=(context,name,"symbolic-dark" if symbol and variant=="dark" else
                 "symbolic-light" if symbol else
                 "dark-action" if variant=="dark" and context=="actions" else
                 "light-action" if variant=="light" else "aero")
            if key not in created:
                asset_dir=root/"_melania-assets"
                asset_dir.mkdir(exist_ok=True)
                asset=asset_dir/(hashlib.sha256(repr(key).encode()).hexdigest()[:28]+".svg")
                img = None if symbol or context=="actions" else original_svg(name,context)
                if img is None:
                    img=generated_svg(name,context,key[2])
                    origins["generated"]+=1
                else:
                    origins["original"]+=1
                ET.fromstring(img)
                asset.write_text(img,encoding="utf8")
                created[key]=asset
            symlink_relative(root/path,created[key])
    # Share colorful scalable illustrations but build contrast-specific
    # symbolic icons for Light. This avoids white-on-white toolbar/status
    # glyphs while retaining the Aero source artwork and reducing duplication.
    main_cats=sorted(CATEGORIES)
    for category in main_cats:
        if category in ("actions","panel"):
            continue
        subdirs=sorted({Path(p).parts[1] for p in paths
                        if Path(p).parts[0]==category and len(Path(p).parts)>2})
        if not subdirs:
            symlink_relative(light/category,dark/category)
        else:
            (light/category).mkdir(exist_ok=True)
            for subdir in subdirs:
                item=light/category/subdir
                if not item.exists():
                    symlink_relative(item,dark/category/subdir)
    # Light on Dark Panel: application dialogs get dark-on-light symbols,
    # while the dark taskbar inherits bright status glyphs and panel icons.
    for category in main_cats:
        source=dark if category in ("panel","status","animations") else light
        symlink_relative(mixed/category,source/category)
    dirs=sorted({str(Path(path).parent) for path in dark_paths+light_paths})
    for root,name,variant in [
        (dark,"Melania Dark","dark"),(light,"Melania Light","light"),
        (mixed,"Melania Light on Dark Panel","mixed")]:
        # Use GTK freedesktop directory indexes consistent with the actual
        # path layout and inherited/symlinked icon directories.
        (root/"index.theme").write_text(index_theme(name,variant,dirs),encoding="utf8")
    return {"dark_identifiers":len(dark_paths),"light_identifiers":len(light_paths),
            "unified_identifiers":len(paths),**origins}

def verify(destination: Path, dark_paths: list[str], light_paths: list[str]):
    for theme in ("Melania-Dark","Melania-Light","Melania-Light-on-Dark-Panel"):
        folder=destination/theme
        missing=[p for p in sorted(set(dark_paths)|set(light_paths))
                 if not (folder/p).is_file()]
        if missing:
            raise AssertionError(f"{theme}: {len(missing)} unresolved identifiers: {missing[:8]}")
        import configparser
        cfg=configparser.ConfigParser(interpolation=None,strict=False)
        cfg.read(folder/"index.theme",encoding="utf8")
        assert cfg["Icon Theme"]["Name"].startswith("Melania ")
    for theme in ("Melania-Dark","Melania-Light","Melania-Light-on-Dark-Panel"):
        for essential in ("folder","user-home","user-trash"):
            assert (destination/theme/f"places/scalable/{essential}.svg").is_file(),(theme,essential)
        assert (destination/theme/"apps/scalable/computer.svg").is_file(), theme

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--output",type=Path,default=ROOT/"dist"/"melania-icon-themes")
    ap.add_argument("--verify",action="store_true")
    args=ap.parse_args()
    dark_paths=inventory("dark")
    light_paths=inventory("light")
    if len(dark_paths)!=20364 or len(light_paths)!=9973:
        raise SystemExit("Unexpected reference manifest size")
    output=args.output.resolve()
    if output.exists():
        if not (output/"Melania-Dark"/"index.theme").is_file():
            raise SystemExit(f"Refusing to replace unrelated output: {output}")
        shutil.rmtree(output)
    output.mkdir(parents=True,exist_ok=True)
    report=build(output,dark_paths,light_paths)
    verify(output,dark_paths,light_paths)
    print("Built and checked Melania Aero themes:", report, "in",output)

if __name__=="__main__": main()
