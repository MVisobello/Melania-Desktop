#!/usr/bin/env python3
"""Original compact Fluent system glyphs for Melania Settings sidebar.

These names previously resolved to generated letter tiles. Each pictogram is
now semantically meaningful and readable at 16/24px. No Microsoft assets.
"""
from __future__ import annotations

TYPES = {
    "bluetooth": "bluetooth",
    "bluetooth-active": "bluetooth",
    "bluetooth-symbolic": "bluetooth",
    "preferences-desktop-theme": "palette",
    "preferences-system-notifications": "notification",
    "preferences-desktop-notification": "notification",
    "applications-other": "apps",
    "avatar-default": "user",
    "x-office-calendar": "calendar",
    "battery": "battery",
    "battery-full": "battery",
    "audio-volume-high": "speaker",
    "audio-card": "speaker",
    "security-high": "shield",
    "preferences-desktop-locale": "globe",
    "preferences-desktop-accessibility": "accessibility",
    "dialog-information": "info",
    "view-grid": "grid",
    "view-grid-symbolic": "grid",
    "input-keyboard": "keyboard",
    "system-search": "search",
}
COLORS = {
    "bluetooth": ("#6cbaff", "#1469cb"),
    "palette": ("#cc88f7", "#7e47c5"),
    "notification": ("#ffcb67", "#e28b1e"),
    "apps": ("#7ab0df", "#346ba4"),
    "user": ("#73bff5", "#146cce"),
    "calendar": ("#fdbd83", "#dd7447"),
    "battery": ("#7edf94", "#26954d"),
    "speaker": ("#9bb1ca", "#456c98"),
    "shield": ("#6dbff4", "#155bbb"),
    "globe": ("#6ae1e6", "#0c8cbd"),
    "accessibility": ("#65c9ff", "#197bce"),
    "info": ("#69c1ff", "#1586d2"),
    "grid": ("#8fc5ef", "#317fb9"),
    "keyboard": ("#a1bdd4", "#4f748f"),
    "search": ("#9cc5ec", "#427cae"),
}
GLYPHS = {
    "bluetooth": '<path d="M47 22v52l19-17-36-25M47 22l19 17-36 25"/>',
    "palette": '<path d="M48 22c18 0 30 11 30 26 0 10-7 16-16 16h-7c-4 0-6 3-6 7 0 4-4 6-9 6-15 0-25-13-25-27 0-16 14-28 33-28Z"/><circle cx="32" cy="40" r="3"/><circle cx="43" cy="31" r="3"/><circle cx="59" cy="34" r="3"/><circle cx="67" cy="48" r="3"/>',
    "notification": '<path d="M30 62q6-8 6-19c0-9 5-15 12-15s12 6 12 15q0 11 7 19ZM42 70q6 10 12 0"/>',
    "apps": '<rect x="25" y="25" width="19" height="19" rx="3"/><rect x="52" y="25" width="19" height="19" rx="3"/><rect x="25" y="52" width="19" height="19" rx="3"/><rect x="52" y="52" width="19" height="19" rx="3"/>',
    "user": '<circle cx="48" cy="36" r="13"/><path d="M22 73q0-21 26-21t26 21"/>',
    "calendar": '<rect x="22" y="29" width="52" height="45" rx="5"/><path d="M22 41h52M35 22v15M61 22v15M32 51h9M48 51h9M32 63h9M48 63h9"/>',
    "battery": '<rect x="19" y="32" width="55" height="37" rx="5"/><path d="M75 43h5v15h-5M28 42h36v17H28Z"/>',
    "speaker": '<path d="M19 41h15l16-13v40L34 55H19Z"/><path d="M59 39q13 10 0 19M68 29q22 20 0 39"/>',
    "shield": '<path d="M48 19q12 9 27 10v15q0 22-27 34-27-12-27-34V29q15-1 27-10Z"/><path d="m35 48 9 9 17-19"/>',
    "globe": '<circle cx="48" cy="48" r="28"/><path d="M20 48h56M48 20q-22 28 0 56M48 20q22 28 0 56"/>',
    "accessibility": '<circle cx="48" cy="24" r="6"/><path d="M24 40h48M48 41v18M48 58l-17 19M48 58l17 19M34 42l-5 16M62 42l5 16"/>',
    "info": '<circle cx="48" cy="48" r="29"/><path d="M48 45v19M48 32v2"/>',
    "grid": '<rect x="22" y="22" width="22" height="22" rx="2"/><rect x="52" y="22" width="22" height="22" rx="2"/><rect x="22" y="52" width="22" height="22" rx="2"/><rect x="52" y="52" width="22" height="22" rx="2"/>',
    "keyboard": '<rect x="13" y="29" width="70" height="40" rx="5"/><path d="M22 41h4M33 41h4M44 41h4M55 41h4M66 41h4M22 51h4M33 51h4M44 51h4M55 51h4M34 60h28"/>',
    "search": '<circle cx="41" cy="41" r="20"/><path d="m57 57 19 19"/>',
}
# Compact muted backgrounds allow icons to remain distinct on both surfaces.
def artwork(name: str, category: str, variant: str = "light", size: int = 96) -> str | None:
    kind = TYPES.get(name)
    if not kind or category not in ("apps", "categories", "places", "devices", "status", "panel", "actions"):
        return None
    top, bottom = COLORS[kind]
    symbolic = name.endswith("-symbolic")
    if symbolic:
        ink = "#f2f8ff" if variant == "dark" else "#244f7b"
        return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">'
                f'<g fill="none" stroke="{ink}" stroke-width="5.5" '
                f'stroke-linecap="round" stroke-linejoin="round">{GLYPHS[kind]}</g></svg>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">'
            '<defs><linearGradient id="face" x2="0" y2="1">'
            f'<stop stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/>'
            '</linearGradient></defs>'
            '<rect x="12" y="12" width="72" height="72" rx="17" fill="url(#face)" '
            'stroke="#174d74" stroke-opacity=".19" stroke-width="2"/>'
            '<path d="M18 34V29q0-12 13-12h33q14 0 14 12v5" '
            'fill="none" stroke="#ffffff" stroke-opacity=".27" stroke-width="5"/>'
            '<g fill="none" stroke="#ffffff" stroke-width="5.5" '
            f'stroke-linecap="round" stroke-linejoin="round">{GLYPHS[kind]}</g></svg>')
