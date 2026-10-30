"""Original compact Melania SVG pictograms for Files and the system tray.

Symbolic icons must be recognizable at 18-22px. No letter tiles,
large gradients, blurry shadows or remapped application illustrations.
Geometry is original and simple (24x24 drawing grid).
"""
from __future__ import annotations

# Icon names used by the native Files sidebar and the lower-right tray.
SIDEBAR = {
    "user-home-symbolic": '<path d="m3 11 9-7 9 7v9H3v-9Z"/><path d="M9 20v-6h6v6"/>',
    "user-desktop-symbolic": '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M12 17v4m-5 0h10"/>',
    "folder-documents-symbolic": '<path d="M2.5 7V5a2 2 0 0 1 2-2H10l2 2h7.5a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2Z"/><path d="M8 11h8m-8 4h6"/>',
    "folder-pictures-symbolic": '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8" cy="9" r="1.5"/><path d="m4 17 5-5 4 3 3-3 5 5"/>',
    "folder-download-symbolic": '<path d="M4 18v3h16v-3M12 3v13m-5-5 5 5 5-5"/>',
    "folder-music-symbolic": '<path d="M10 17V5l10-2v12"/><ellipse cx="7" cy="18" rx="3" ry="2"/><ellipse cx="17" cy="16" rx="3" ry="2"/>',
    "folder-videos-symbolic": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m10 9 5 3-5 3Z"/>',
    "drive-harddisk-symbolic": '<rect x="3" y="6" width="18" height="13" rx="3"/><path d="M5 14h14M7 16.5h.1M11 16.5h.1"/>',
    "user-trash-symbolic": '<path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6m4-6v6"/>',
    "user-bookmarks-symbolic": '<path d="M6 3h12v18l-6-4-6 4Z"/>',
}
TRAY = {
    "network-wireless-signal-excellent-symbolic": '<path d="M2 9c5.7-5.1 14.3-5.1 20 0M5.5 12.5c3.7-3.2 9.3-3.2 13 0M9 16c1.7-1.5 4.3-1.5 6 0"/><circle cx="12" cy="20" r="1" fill="currentColor" stroke="none"/>',
    "bluetooth-active-symbolic": '<path d="M12 2v20l6-5-12-10m6-5 6 5L6 17"/>',
    "audio-volume-high-symbolic": '<path d="M3 9h4l5-4v14l-5-4H3Z"/><path d="M15 9a5 5 0 0 1 0 6m3-9a9 9 0 0 1 0 12"/>',
    "battery-good-symbolic": '<rect x="2" y="7" width="18" height="10" rx="2"/><path d="M22 10v4M6 10h10v4H6Z"/>',
    "preferences-system-notifications-symbolic": '<path d="M6 17h12l-2-3V10a4 4 0 0 0-8 0v4Z"/><path d="M10 20c1 2 3 2 4 0"/>',
}


def artwork(name: str, category: str, variant: str = "light", size: int = 24) -> str | None:
    if not name.endswith("-symbolic"):
        return None
    if name in SIDEBAR:
        if category not in ("places", "apps", "devices", "actions"):
            return None
        stroke = "#d8e1ec" if variant == "dark" else "#30445a"
        shape = SIDEBAR[name]
    elif name in TRAY:
        if category not in ("status", "panel", "apps", "actions"):
            return None
        stroke = "#e6f3ff" if variant == "dark" else "#294c70"
        shape = TRAY[name]
    else:
        return None
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="{size}" height="{size}">'
            f'<g fill="none" stroke="{stroke}" stroke-width="1.8" stroke-linecap="round" '
            f'stroke-linejoin="round">{shape}</g></svg>')
