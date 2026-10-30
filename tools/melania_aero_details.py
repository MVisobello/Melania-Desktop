#!/usr/bin/env python3
"""Melania Aero finishing layer: pixel-aware, recognisable system icons.

Original vector illustration only. Flat Remix is never read or embedded.
Compact glyphs use deliberately heavier geometry at <=24 physical pixels.
"""
from __future__ import annotations

from melania_aero_artwork import ACTIONS, DEVICES, FILES, FOLDERS, action, device, document, folder, svg

# Real freedesktop/GTK names that are particularly visible in Files and Panel.
EXTRA_FILES = {
    "application-x-pdf": "pdf", "application-postscript": "pdf",
    "application-msword": "document", "application-vnd.oasis.opendocument.text": "document",
    "application-vnd.openxmlformats-officedocument.wordprocessingml.document": "document",
    "application-vnd.ms-excel": "spreadsheet",
    "application-vnd.oasis.opendocument.spreadsheet": "spreadsheet",
    "application-vnd.openxmlformats-officedocument.spreadsheetml.sheet": "spreadsheet",
    "application-vnd.ms-powerpoint": "presentation",
    "application-vnd.oasis.opendocument.presentation": "presentation",
    "application-vnd.openxmlformats-officedocument.presentationml.presentation": "presentation",
    "text-html": "executable", "text-x-script": "executable",
    "text-x-python": "executable", "text-x-csrc": "executable",
    "application-x-shellscript": "executable", "application-x-sharedlib": "executable",
    "application-x-compressed-tar": "archive", "application-x-tar": "archive",
    "application-x-7z-compressed": "archive", "application-x-rar": "archive",
    "application-x-iso": "archive", "application-x-cd-image": "archive",
    "font-x-generic": "text", "application-x-font-ttf": "text",
}
EXTRA_DEVICES = {
    "computer-desktop": "computer", "computer-notebook": "laptop",
    "drive-harddisk-system": "disk", "drive-harddisk-usb": "disk",
    "drive-removable-media-usb": "usb", "drive-multidisk": "disk",
    "printer-network": "printer", "printer-printing": "printer",
    "media-flash": "usb", "input-touchpad": "mouse",
}
EXTRA_FOLDERS = {
    "folder-templates": "documents", "folder-public": "public",
    "folder-shared": "public", "folder-image": "pictures",
    "folder-photo": "pictures", "folder-video": "videos",
    "folder-audio": "music", "folder-development": "documents",
    "folder-bookmarks": "documents", "folder-favorites": "documents",
}
EXTRA_ACTIONS = {
    "edit-undo": "undo", "edit-redo": "redo",
    "document-print": "print", "document-properties": "properties",
    "view-list": "list", "view-grid": "grid",
    "view-list-symbolic": "list", "view-grid-symbolic": "grid",
    "folder-new": "folder-new", "tab-new": "new-tab",
    "window-new": "new-tab", "view-fullscreen": "expand",
    "view-restore": "restore", "media-playback-start": "play",
    "media-playback-pause": "pause", "media-playback-stop": "stop",
    "media-skip-forward": "skip-next", "media-skip-backward": "skip-back",
    "system-search": "search", "system-log-out": "logout",
    "system-shutdown": "power", "preferences-system": "settings",
}
AERO_ACTIONS = {
    "undo": '<path d="m39 28-21 20 21 18M20 48h37q18 0 18 17"/>',
    "redo": '<path d="m57 28 21 20-21 18M76 48H39q-18 0-18 17"/>',
    "print": '<path d="M30 18h36v21H30ZM21 36h54v31H21ZM30 55h36v23H30Z"/>',
    "properties": '<path d="M25 17h33l14 14v48H25ZM58 17v14h14M35 48h25M35 57h21M35 66h27"/>',
    "list": '<path d="M33 29h43M33 48h43M33 67h43"/><circle cx="21" cy="29" r="3"/><circle cx="21" cy="48" r="3"/><circle cx="21" cy="67" r="3"/>',
    "grid": '<rect x="19" y="19" width="25" height="25" rx="3"/><rect x="52" y="19" width="25" height="25" rx="3"/><rect x="19" y="52" width="25" height="25" rx="3"/><rect x="52" y="52" width="25" height="25" rx="3"/>',
    "folder-new": '<path d="M15 32h27l9 8h29l-7 34H18Z"/><path d="M63 18v23M51 29h24"/>',
    "new-tab": '<rect x="18" y="24" width="61" height="51" rx="4"/><path d="M18 36h61M48 47v21M38 57h20"/>',
    "expand": '<path d="M40 20H20v20M20 20l25 25M56 20h20v20M76 20 51 45M20 56v20h20M20 76l25-25M76 56v20H56M76 76 51 51"/>',
    "restore": '<path d="M18 40h22V18M18 18l22 22M78 40H56V18M78 18 56 40M18 56h22v22M18 78l22-22M78 56H56v22M78 78 56 56"/>',
    "play": '<path d="m32 18 42 30-42 30Z"/>',
    "pause": '<rect x="26" y="22" width="14" height="52" rx="2"/><rect x="56" y="22" width="14" height="52" rx="2"/>',
    "stop": '<rect x="26" y="26" width="44" height="44" rx="4"/>',
    "skip-next": '<path d="m25 25 31 23-31 23ZM70 24v48"/>',
    "skip-back": '<path d="M71 25 40 48l31 23ZM26 24v48"/>',
    "search": '<circle cx="43" cy="41" r="19"/><path d="m57 57 20 20"/>',
    "logout": '<path d="M43 22H23v52h20M50 48h30M64 32l16 16-16 16"/>',
    "power": '<path d="M48 17v34M30 29a28 28 0 1 0 36 0"/>',
    "settings": '<circle cx="48" cy="48" r="27" stroke-dasharray="12 6"/><circle cx="48" cy="48" r="11"/>',
}
CATEGORY_MARKS = {
    "applications-internet": "globe",
    "applications-network": "globe",
    "applications-office": "office",
    "applications-graphics": "graphics",
    "applications-multimedia": "multimedia",
    "applications-utilities": "utilities",
    "applications-system": "system",
    "applications-development": "development",
    "applications-accessories": "utilities",
    "applications-science": "science",
    "applications-education": "science",
    "applications-games": "games",
}
CATEGORY_PATHS = {
    "globe": '<circle cx="48" cy="47" r="27"/><path d="M21 47h54M48 20q-22 27 0 54M48 20q22 27 0 54"/>',
    "office": '<path d="M28 22h31l10 11v42H28ZM59 22v11h10M36 43h25M36 52h25M36 61h19"/>',
    "graphics": '<path d="M20 69q24-10 28-46 25 6 29 27 3 24-29 24Z"/><circle cx="44" cy="36" r="3"/><circle cx="58" cy="44" r="3"/><circle cx="59" cy="59" r="3"/>',
    "multimedia": '<path d="M30 23v48l39-24Z"/><path d="M21 21h54v53H21Z"/>',
    "utilities": '<path d="M30 26 68 64M66 22 24 64M20 68l8 8M62 18l16 16"/>',
    "system": '<circle cx="48" cy="48" r="22" stroke-dasharray="10 5"/><circle cx="48" cy="48" r="10"/>',
    "development": '<path d="m38 28-19 20 19 20M58 28l19 20-19 20M53 21 43 75"/>',
    "science": '<path d="M38 20h20M43 20v24L23 72q-5 8 5 8h40q10 0 5-8L53 44V20M35 61h26"/>',
    "games": '<path d="M27 39h42q11 0 14 18l2 13q0 11-11 7L61 65H35L22 77q-11 4-11-7l2-13q3-18 14-18Z"/><path d="M26 53h17M35 44v18"/><circle cx="61" cy="52" r="3"/><circle cx="71" cy="59" r="3"/>',
}


def _compact_folder(kind: str) -> str:
    # Broad, high-contrast silhouette: the large illustration has thinner
    # bevels, but at 16px these would create noise and muddy the folder edge.
    colors = '<linearGradient id="gold" x2="0" y2="1"><stop stop-color="#fff2a9"/><stop offset="1" stop-color="#d8a03b"/></linearGradient>'
    content = ('<path d="M8 29q0-7 8-7h23l8 9h32q8 0 8 8v33q0 8-8 8H15q-7 0-7-8Z" '
               'fill="#dca443" stroke="#a27629" stroke-width="3"/>'
               '<path d="M10 42h76L79 80H13q-7 0-6-8Z" fill="url(#gold)" stroke="#a77624" stroke-width="2.5"/>'
               '<path d="M14 44h68" stroke="#fff4c5" stroke-width="4" opacity=".9"/>')
    # Single, unmistakable emblem; small details from full art are suppressed.
    glyph = {
        "home": 'M36 60l12-10 12 10v12H36Z',
        "documents": 'M37 51h22v21H37Z',
        "download": 'M48 48v22m-9-9 9 9 9-9',
        "music": 'M44 67V51l15-4v19',
        "pictures": 'M34 52h29v20H34Z',
        "videos": 'm39 53 22 10-22 10Z',
        "desktop": 'M35 53h27v18H35Z',
        "public": 'M36 70q0-15 12-15t12 15',
        "remote": 'M34 62a14 14 0 1 0 28 0 14 14 0 1 0-28 0',
    }.get(kind)
    if glyph:
        content += f'<path d="{glyph}" fill="none" stroke="#78521d" stroke-width="4.5" stroke-linejoin="round" stroke-linecap="round"/>'
    return svg(content, colors)


def _compact_document(kind: str) -> str:
    colors = {"pdf": "#cc4442", "document": "#337ab6", "spreadsheet": "#258d5d",
              "presentation": "#d18a37", "archive": "#b48c38", "audio": "#8b6cba",
              "image": "#318cba", "video": "#5276ab", "executable": "#416f9a",
              "text": "#477da7"}
    color = colors[kind]
    return svg('<path d="M25 10h37l13 14v61H25Z" fill="#f2f8ff" stroke="#718da5" stroke-width="3"/>'
               '<path d="M62 10v14h13Z" fill="#c4dbf1" stroke="#718da5" stroke-width="2"/>'
               f'<path d="M30 42h40v24H30Z" fill="{color}"/>'
               '<path d="M37 50h25M37 58h20" fill="none" stroke="white" stroke-width="4"/>'
               '<path d="M33 74h27" stroke="#8ca9bf" stroke-width="3"/>')


def _status_kind(name: str):
    n = name.lower().replace("_", "-")
    if "wireless" in n or "wifi" in n: return "wifi"
    if "audio-volume" in n or "volume-" in n: return "volume"
    if "battery" in n: return "battery"
    if "bluetooth" in n: return "bluetooth"
    if "network-wired" in n or "network-ethernet" in n: return "wired"
    if "microphone" in n or "audio-input" in n: return "mic"
    return None


def _status(name: str, variant: str) -> str:
    kind = _status_kind(name)
    dark = variant == "dark"
    ink = "#f1f9ff" if dark else "#24476c"
    accent = "#66caff" if dark else "#167ebc"
    muted = any(word in name for word in ("muted", "disabled", "off", "offline", "empty"))
    shape = {
        "wifi": '<path d="M13 36q35-31 70 0M25 48q23-21 46 0M37 60q11-10 22 0"/><circle cx="48" cy="72" r="4" fill="currentColor" stroke="none"/>',
        "volume": '<path d="M15 41h15l17-14v43L30 55H15Z"/><path d="M56 39q13 9 0 19M65 29q24 19 0 39"/>',
        "battery": '<rect x="18" y="29" width="58" height="38" rx="6"/><path d="M77 40h6v16h-6"/><rect x="24" y="35" width="39" height="26" rx="2" fill="currentColor" stroke="none"/>',
        "bluetooth": '<path d="M45 18v60l21-20-35-28M45 18l21 20-35 28"/>',
        "wired": '<path d="M27 24h42v20H27ZM34 44v12M48 44v12M62 44v12M19 57h58v16H19Z"/>',
        "mic": '<rect x="38" y="16" width="20" height="42" rx="10"/><path d="M27 46q0 23 21 23t21-23M48 69v12M36 81h24"/>',
    }[kind]
    if kind == "battery":
        if "empty" in name or "low" in name:
            shape = shape.replace('width="39"', 'width="9"')
        elif "half" in name or "50" in name:
            shape = shape.replace('width="39"', 'width="20"')
    if muted:
        shape += '<path d="m66 65 18 18M84 65 66 83" stroke="#e36869" stroke-width="7"/>'
    return svg(f'<g fill="none" color="{accent}" stroke="{ink}" stroke-width="6.5" '
               f'stroke-linecap="round" stroke-linejoin="round">{shape}</g>')


def refined_artwork(name: str, category: str, variant: str, symbolic: bool,
                    size: int = 96) -> str | None:
    compact = size <= 24
    if category in ("status", "panel") and _status_kind(name):
        return _status(name, variant)
    # Symbolic actions need a high contrast glyph without gradients or badges.
    if category == "actions" and name.endswith("-symbolic"):
        stem = name.removesuffix("-symbolic")
        if stem in ACTIONS:
            name = stem
        else:
            return None
    elif symbolic:
        return None
    if category == "actions" and name in EXTRA_ACTIONS:
        kind = EXTRA_ACTIONS[name]
        dark = variant == "dark"
        ink = "#f1f8ff" if dark else "#285074"
        accent = "#62cdff" if dark else "#177cbe"
        defs = f'<linearGradient id="a" x2="0" y2="1"><stop stop-color="{accent}"/><stop offset="1" stop-color="{ink}"/></linearGradient>'
        return svg('<g fill="none" stroke="url(#a)" stroke-linecap="round" '
                   'stroke-linejoin="round" stroke-width="' + ("8" if compact else "5.5") +
                   '">' + AERO_ACTIONS[kind] + '</g>', defs)
    if category == "actions" and name in ACTIONS and compact:
        return action(ACTIONS[name], variant).replace('stroke-width="5.5"', 'stroke-width="8"')
    if category == "places":
        kind = FOLDERS.get(name, EXTRA_FOLDERS.get(name))
        if kind and compact:
            return _compact_folder(kind)
        if name in EXTRA_FOLDERS:
            return folder(EXTRA_FOLDERS[name])
    if category == "mimetypes":
        kind = FILES.get(name, EXTRA_FILES.get(name))
        if kind and compact:
            return _compact_document(kind)
        if name in EXTRA_FILES:
            return document(EXTRA_FILES[name])
    if category == "devices":
        kind = DEVICES.get(name, EXTRA_DEVICES.get(name))
        if kind and name in EXTRA_DEVICES:
            return device(kind)
    if category == "categories" and name in CATEGORY_MARKS:
        kind = CATEGORY_MARKS[name]
        # Category blue/cyan vitreous medallion, distinct icon motif for each category.
        defs = ('<linearGradient id="g" x2="0" y2="1"><stop stop-color="#9ce4ff"/>'
                '<stop offset=".5" stop-color="#298ccd"/><stop offset="1" stop-color="#135286"/></linearGradient>')
        return svg('<circle cx="48" cy="47" r="38" fill="url(#g)" stroke="#175078" stroke-width="2"/>'
                   '<path d="M17 31q14-24 43-17" fill="none" stroke="white" stroke-opacity=".55" stroke-width="5"/>'
                   '<g fill="none" stroke="white" stroke-linejoin="round" stroke-linecap="round" '
                   'stroke-width="' + ("7" if compact else "5") + '">' +
                   CATEGORY_PATHS[kind] + '</g>', defs)
    return None


def is_curated(name: str, category: str) -> bool:
    if category == "actions":
        return name in ACTIONS or name in EXTRA_ACTIONS
    if category == "places":
        return name in FOLDERS or name in EXTRA_FOLDERS
    if category == "mimetypes":
        return name in FILES or name in EXTRA_FILES
    if category == "devices":
        return name in DEVICES or name in EXTRA_DEVICES
    if category == "categories":
        return name in CATEGORY_MARKS
    return False
