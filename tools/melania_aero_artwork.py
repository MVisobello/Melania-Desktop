#!/usr/bin/env python3
"""Hand-crafted Melania Aero icon artwork. Original vector shapes, no Flat Remix art.

This is a small curated core, not a claim that every freedesktop alias has
independent art. Every icon uses a 96x96 viewBox and remains transparent.
"""
from __future__ import annotations
from html import escape

# Named source artwork overrides apply only to these exact standard identifiers.
FOLDERS = {
    "folder": "plain", "folder-open": "open", "folder-documents": "documents",
    "folder-download": "download", "folder-downloads": "download",
    "folder-music": "music", "folder-pictures": "pictures",
    "folder-videos": "videos", "folder-desktop": "desktop",
    "folder-publicshare": "public", "folder-remote": "remote",
    "folder-network": "remote", "user-home": "home",
    "user-desktop": "desktop", "user-documents": "documents",
    "user-download": "download", "user-music": "music",
    "user-pictures": "pictures", "user-videos": "videos",
}
FILES = {
    "text-x-generic": "text", "application-pdf": "pdf",
    "x-office-document": "document", "x-office-spreadsheet": "spreadsheet",
    "x-office-presentation": "presentation", "image-x-generic": "image",
    "audio-x-generic": "audio", "video-x-generic": "video",
    "application-x-executable": "executable", "application-zip": "archive",
    "package-x-generic": "archive", "application-x-archive": "archive",
}
DEVICES = {
    "computer": "computer", "computer-laptop": "laptop",
    "video-display": "display", "drive-harddisk": "disk",
    "drive-removable-media": "usb", "media-removable": "usb",
    "printer": "printer", "input-keyboard": "keyboard",
    "input-mouse": "mouse", "audio-headphones": "headphones",
}
ACTIONS = {
    "go-previous": "back", "go-next": "next", "go-up": "up",
    "go-down": "down", "go-home": "home", "view-refresh": "refresh",
    "edit-copy": "copy", "edit-cut": "cut", "edit-paste": "paste",
    "document-open": "open", "document-save": "save",
    "document-new": "new", "edit-delete": "delete",
    "edit-find": "find", "zoom-in": "plus", "zoom-out": "minus",
    "window-close": "close", "list-add": "plus", "list-remove": "minus",
}
ACTION_PATHS = {
    "back": '<path d="M58 24 34 48l24 24M35 48h43"/>',
    "next": '<path d="m38 24 24 24-24 24M61 48H18"/>',
    "up": '<path d="M24 57 48 33l24 24M48 34v43"/>',
    "down": '<path d="m24 39 24 24 24-24M48 61V19"/>',
    "home": '<path d="m17 45 31-26 31 26M26 42v34h44V42M41 76V56h14v20"/>',
    "refresh": '<path d="M72 41A25 25 0 0 0 27 35m0-13v17h17M24 55A25 25 0 0 0 69 61m0 13V57H52"/>',
    "copy": '<rect x="17" y="18" width="44" height="44" rx="4"/><rect x="32" y="32" width="44" height="44" rx="4"/>',
    "cut": '<circle cx="27" cy="68" r="8"/><circle cx="70" cy="68" r="8"/><path d="m29 26 38 35M67 26 29 61"/>',
    "paste": '<rect x="25" y="28" width="46" height="51" rx="5"/><path d="M37 29v-8h23v8M36 46h24M36 56h21"/>',
    "open": '<path d="M16 31h29l8 9h27l-8 34H22Z"/><path d="M16 31v-7h27l7 8"/>',
    "save": '<path d="M24 18h43l10 11v50H24Z"/><path d="M36 18v22h29V18M35 78V56h32v22"/>',
    "new": '<path d="M24 15h34l14 14v50H24ZM58 15v14h14M36 52h25M48 40v25"/>',
    "delete": '<path d="M29 31h39l-5 45H34ZM23 31h51M39 23h19M43 43v22M55 43v22"/>',
    "find": '<circle cx="41" cy="42" r="19"/><path d="m56 57 20 20"/>',
    "plus": '<path d="M48 22v52M22 48h52"/>',
    "minus": '<path d="M22 48h52"/>',
    "close": '<path d="m28 28 40 40M68 28 28 68"/>',
}


def svg(content: str, defs: str = "") -> str:
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" '
            'viewBox="0 0 96 96"><defs>' + defs + '</defs>' + content + '</svg>')


def folder(kind: str) -> str:
    defs = """<linearGradient id="back" x2="0" y2="1"><stop stop-color="#f9df88"/>
<stop offset=".52" stop-color="#e9b748"/><stop offset="1" stop-color="#bb8629"/></linearGradient>
<linearGradient id="front" x2=".2" y2="1"><stop stop-color="#fff3ae"/>
<stop offset=".42" stop-color="#f3c75a"/><stop offset="1" stop-color="#cc922f"/></linearGradient>"""
    back = ('<path d="M8 28q0-7 7-7h23l9 9h33q8 0 8 8v34q0 7-8 7H16q-8 0-8-8Z" '
            'fill="url(#back)" stroke="#a97720" stroke-width="1.4"/>')
    front = ('<path d="M10 40q2-6 9-6h66q7 0 5 8l-8 35q-1 5-8 5H14q-8 0-7-7Z" '
             'fill="url(#front)" stroke="#ac7824" stroke-width="1.5"/>'
             '<path d="M16 41h67L79 49H13Z" fill="#fff6c4" opacity=".6"/>')
    if kind == "open":
        front = ('<path d="M17 44 91 36 77 81H7Z" fill="url(#front)" '
                 'stroke="#a9792a" stroke-width="1.5"/>'
                 '<path d="m18 46 65-7-3 7-64 7Z" fill="#fff2ba" opacity=".6"/>')
    emblem = {
        "home": '<path d="m36 58 12-10 12 10v12H36ZM44 70V60h8v10" />',
        "documents": '<path d="M38 50h18l6 6v17H38ZM56 50v7h6M43 62h14M43 67h12"/>',
        "download": '<path d="M48 49v19m-8-8 8 8 8-8M37 72h22"/>',
        "music": '<path d="M45 69V54l15-4v15M45 58l15-4"/><ellipse cx="40" cy="70" rx="5" ry="3"/><ellipse cx="55" cy="66" rx="5" ry="3"/>',
        "pictures": '<rect x="34" y="51" width="29" height="22" rx="2"/><circle cx="55" cy="57" r="2"/><path d="m36 69 9-9 6 5 4-3 6 7"/>',
        "videos": '<rect x="34" y="52" width="29" height="20" rx="3"/><path d="m45 56 10 6-10 6Z"/>',
        "desktop": '<rect x="33" y="51" width="31" height="20" rx="2"/><path d="M48 71v5M40 76h16"/>',
        "public": '<circle cx="43" cy="57" r="4"/><circle cx="56" cy="57" r="4"/><path d="M35 70q0-9 8-9t8 9M48 70q0-9 8-9t8 9"/>',
        "remote": '<circle cx="48" cy="63" r="12"/><path d="M36 63h24M48 51q-9 12 0 24M48 51q9 12 0 24"/>',
    }.get(kind, "")
    overlay = ('<g fill="none" stroke="#8a621c" stroke-width="2.4" '
               'stroke-linejoin="round" stroke-linecap="round">' + emblem + '</g>') if emblem else ""
    return svg(back + front + overlay, defs)


def document(kind: str) -> str:
    defs = """<linearGradient id="paper" x2="0" y2="1">
<stop stop-color="#ffffff"/><stop offset=".6" stop-color="#e7f3ff"/>
<stop offset="1" stop-color="#c3d9eb"/></linearGradient>"""
    colors = {"pdf": "#cb443d", "document": "#337bcc", "spreadsheet": "#2f9b69",
              "presentation": "#e58d32", "image": "#6489c7",
              "audio": "#8b76bc", "video": "#5a81aa", "archive": "#b89542",
              "executable": "#4b759d", "text": "#4982b9"}
    tint = colors[kind]
    page = ('<path d="M23 10h35l15 16v60H23Z" fill="url(#paper)" stroke="#7894b3" stroke-width="1.6"/>'
            '<path d="M58 10v16h15Z" fill="#d0e6f7" stroke="#7894b3" stroke-width="1.3"/>'
            '<path d="M29 16h24" stroke="#fff" stroke-width="2.5"/>')
    marks = {
        "text": '<path d="M33 41h31M33 50h31M33 59h26M33 68h29"/>',
        "pdf": '<path d="M38 66q18-35 17-18t-17 18q-12 4 4-6l18-6"/>',
        "document": '<path d="M37 45h25M37 53h25M37 61h23M37 69h25"/>',
        "spreadsheet": '<rect x="33" y="40" width="30" height="32" rx="1"/><path d="M33 50h30M33 61h30M43 40v32M53 40v32"/>',
        "presentation": '<rect x="32" y="41" width="32" height="25" rx="2"/><path d="m37 60 7-10 5 6 5-8 6 12M48 66v9"/>',
        "image": '<rect x="32" y="40" width="32" height="29" rx="2"/><circle cx="55" cy="48" r="3"/><path d="m35 65 10-11 7 7 5-5 5 9"/>',
        "audio": '<path d="M42 61V44l17-4v17M42 48l17-4"/><ellipse cx="37" cy="63" rx="5" ry="3"/><ellipse cx="54" cy="59" rx="5" ry="3"/>',
        "video": '<rect x="32" y="41" width="32" height="29" rx="3"/><path d="m44 47 13 9-13 9Z"/>',
        "archive": '<path d="M48 37v29M43 39h10M43 47h10M43 55h10M43 63h10"/>',
        "executable": '<path d="M36 50 45 59 36 68M49 68h13"/>',
    }[kind]
    return svg(page + '<g fill="none" stroke="' + tint +
               '" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">' +
               marks + '</g>', defs)


def device(kind: str) -> str:
    defs = """<linearGradient id="metal" x2="0" y2="1">
<stop stop-color="#f7fbff"/><stop offset=".52" stop-color="#b5c8d9"/>
<stop offset="1" stop-color="#63809a"/></linearGradient>
<linearGradient id="screen" x2="0" y2="1">
<stop stop-color="#87d7ff"/><stop offset=".55" stop-color="#338bca"/>
<stop offset="1" stop-color="#16518b"/></linearGradient>"""
    shapes = {
        "computer": '<rect x="12" y="13" width="72" height="53" rx="6" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><rect x="17" y="18" width="62" height="41" rx="2" fill="url(#screen)"/><path d="M48 66v12M32 79h32" stroke="#728fa7" stroke-width="6" stroke-linecap="round"/>',
        "display": '<rect x="9" y="17" width="78" height="50" rx="5" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><rect x="14" y="21" width="68" height="41" fill="url(#screen)"/><path d="M48 67v11M31 79h34" stroke="#637f99" stroke-width="6" stroke-linecap="round"/>',
        "laptop": '<rect x="18" y="16" width="60" height="48" rx="4" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><rect x="23" y="21" width="50" height="37" fill="url(#screen)"/><path d="M18 64h60l10 12H8Z" fill="url(#metal)" stroke="#526b82" stroke-width="2"/>',
        "disk": '<rect x="18" y="15" width="60" height="65" rx="9" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><circle cx="48" cy="44" r="20" fill="#dce8f2" stroke="#879ead" stroke-width="2"/><circle cx="48" cy="44" r="6" fill="#7d9eb6"/><circle cx="67" cy="68" r="3" fill="#39b38c"/>',
        "usb": '<rect x="27" y="10" width="42" height="76" rx="8" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><rect x="34" y="17" width="28" height="21" rx="2" fill="#779cb7"/><path d="M41 22v10M55 22v10" stroke="#d9e9f5" stroke-width="5"/>',
        "printer": '<rect x="27" y="12" width="42" height="28" fill="#edf7ff" stroke="#7794aa" stroke-width="2"/><rect x="15" y="34" width="66" height="38" rx="6" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><rect x="28" y="54" width="40" height="30" fill="#f9fcff" stroke="#7794aa" stroke-width="2"/><path d="M35 63h26M35 71h23" stroke="#9eb8ca" stroke-width="2"/>',
        "keyboard": '<rect x="7" y="31" width="82" height="43" rx="6" fill="url(#metal)" stroke="#526b82" stroke-width="2"/>' + ''.join(f'<rect x="{x}" y="{y}" width="9" height="7" rx="1.5" fill="#536c83"/>' for y in (38,49) for x in range(13,81,12)) + '<rect x="27" y="60" width="44" height="7" rx="2" fill="#536c83"/>',
        "mouse": '<path d="M48 13q24 0 24 25v20q0 25-24 25T24 58V38q0-25 24-25Z" fill="url(#metal)" stroke="#526b82" stroke-width="2"/><path d="M48 14v30M25 44h46" stroke="#7693ac" stroke-width="2"/>',
        "headphones": '<path d="M19 56V43a29 29 0 0 1 58 0v13" fill="none" stroke="#607e99" stroke-width="11"/><rect x="13" y="50" width="16" height="27" rx="6" fill="url(#metal)" stroke="#526b82"/><rect x="67" y="50" width="16" height="27" rx="6" fill="url(#metal)" stroke="#526b82"/>',
    }
    return svg(shapes[kind], defs)


def action(kind: str, variant: str) -> str:
    # Standalone shapes: no tiles behind compact toolbar icons.
    dark = variant == "dark"
    primary = "#edf8ff" if dark else "#24557d"
    accent = "#66ceff" if dark else "#188bd2"
    defs = (f'<linearGradient id="a" x2="0" y2="1"><stop stop-color="{accent}"/>'
            f'<stop offset="1" stop-color="{primary}"/></linearGradient>')
    return svg('<g fill="none" stroke="url(#a)" stroke-width="5.5" '
               'stroke-linecap="round" stroke-linejoin="round">' +
               ACTION_PATHS[kind] + '</g>', defs)


def artwork(name: str, category: str, variant: str, symbolic: bool) -> str | None:
    # Symbolic icons keep the dedicated high-contrast builder; these drawings
    # are used only for full-color entities and explicitly drawn toolbar actions.
    if symbolic:
        return None
    if category == "places" and name in FOLDERS:
        return folder(FOLDERS[name])
    if category == "mimetypes" and name in FILES:
        return document(FILES[name])
    if category == "devices" and name in DEVICES:
        return device(DEVICES[name])
    if category == "actions" and name in ACTIONS:
        return action(ACTIONS[name], variant)
    return None
