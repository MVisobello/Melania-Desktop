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
# Compact original status glyphs with a consistent 2.2px stroke, strong
# visual balance and explicit colored fills (no currentColor ambiguity).
# The melania-tray-* names are the default names for the native taskbar.
# Keep the freedesktop aliases so other consumers do not lose icons.
WIFI = ('<path d="M2.5 8.5c5.3-4.7 13.7-4.7 19 0" />'
        '<path d="M6 12.4c3.3-2.9 8.7-2.9 12 0" />'
        '<path d="M9.5 16.2c1.4-1.2 3.6-1.2 5 0" />'
        '<circle cx="12" cy="20.2" r="1.25" fill="{ink}" stroke="none"/>')
BLUETOOTH = ('<circle cx="12" cy="12" r="10.1" />'
             '<path d="M11.4 4.7v14.6l4.7-3.8-8-6.7m3.3-4.1 4.7 3.8-8 6.7" />')
VOLUME = ('<path d="M2.5 9.2h4.6l5.1-4.1v13.8l-5.1-4.1H2.5z" '
          'fill="{ink}" stroke="none"/>'
          '<path d="M15 8.8c2.5 1.8 2.5 4.6 0 6.4" />'
          '<path d="M18 5.6c4.4 3.4 4.4 9.4 0 12.8" />')
BATTERY = ('<rect x="2.3" y="7.1" width="17.6" height="9.8" rx="2.1"/>'
           '<path d="M21.5 10.2v3.6" />'
           '<rect x="5.2" y="10" width="5.7" height="4" rx=".8" '
           'fill="{ink}" stroke="none"/>')
BELL = ('<path d="M5.6 17.3h12.8l-2.2-3.1V9.6'
        'a4.2 4.2 0 0 0-8.4 0v4.6z" />'
        '<path d="M10 20.2c.7 1.4 3.3 1.4 4 0" />')
TRAY = {
    "melania-tray-wifi-symbolic": WIFI,
    "melania-tray-bluetooth-symbolic": BLUETOOTH,
    "melania-tray-volume-symbolic": VOLUME,
    "melania-tray-battery-symbolic": BATTERY,
    "melania-tray-notifications-symbolic": BELL,
    "network-wireless-signal-excellent-symbolic": WIFI,
    "bluetooth-active-symbolic": BLUETOOTH,
    "audio-volume-high-symbolic": VOLUME,
    "battery-good-symbolic": BATTERY,
    "preferences-system-notifications-symbolic": BELL,
}

# Small GTK widget affordances must not degrade to unknown-icon squares.
WIDGETS = {
    "pan-down-symbolic": '<path d="m6 9 6 6 6-6"/>',
    "pan-end-symbolic": '<path d="m9 6 6 6-6 6"/>',
    "list-add-symbolic": '<path d="M12 4v16M4 12h16"/>',
    "view-sort-ascending-symbolic": '<path d="M5 19V5m-3 3 3-3 3 3M11 7h10M11 12h7M11 17h4"/>',
    "view-list-symbolic": '<path d="M9 6h12M9 12h12M9 18h12M4 6h.1M4 12h.1M4 18h.1"/>',
    "view-more-symbolic": '<circle cx="5" cy="12" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="19" cy="12" r="1.5"/>',
    "edit-select-all-symbolic": '<rect x="5" y="5" width="14" height="14" rx="2"/><path d="m9 12 2 2 4-4"/>',
    "edit-cut-symbolic": '<circle cx="5" cy="6" r="2.5"/><circle cx="5" cy="18" r="2.5"/><path d="m7 8 12 12M7 16 19 4"/>',
    "edit-copy-symbolic": '<rect x="7" y="7" width="13" height="14" rx="2"/><path d="M16 7V4a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h2"/>',
    "edit-paste-symbolic": '<rect x="5" y="5" width="14" height="17" rx="2"/><rect x="9" y="2" width="6" height="5" rx="1"/><path d="M9 12h6m-6 4h5"/>',
    "edit-delete-symbolic": '<path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6m4-6v6"/>',
    "go-previous-symbolic": '<path d="m14 5-7 7 7 7"/>',
    "go-next-symbolic": '<path d="m10 5 7 7-7 7"/>',
    "go-up-symbolic": '<path d="m5 14 7-7 7 7"/>',
    "go-home-symbolic": '<path d="m3 11 9-7 9 7v9H3v-9Z"/><path d="M9 20v-6h6v6"/>',
    "view-refresh-symbolic": '<path d="M20 10a8 8 0 1 0 1 6M20 4v6h-6"/>',
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
        stroke = "#f1f7fe" if variant == "dark" else "#365675"
        shape = TRAY[name]
    elif name in WIDGETS:
        if category not in ("actions", "apps"):
            return None
        stroke = "#e6f3ff" if variant == "dark" else "#30445a"
        shape = WIDGETS[name]
    else:
        return None
    weight = ("1.8" if "bluetooth" in name else "2.15") if name in TRAY else "1.8"
    shape = shape.replace("{ink}", stroke)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="{size}" height="{size}">'
            f'<g fill="none" stroke="{stroke}" stroke-width="{weight}" stroke-linecap="round" '
            f'stroke-linejoin="round">{shape}</g></svg>')
