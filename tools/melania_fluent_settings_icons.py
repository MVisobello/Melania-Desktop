"""Original, non-lettered Melania Fluent glyphs for system settings.

These compact SVG pictograms cover freedesktop names not yet represented by
the approved reference icon illustrations. The themed icons have consistent
96px geometry and remain legible at Settings' 19-21px sizes.
"""
from __future__ import annotations

# Geometry is original: no SVGs copied from GNOME, Microsoft or upstream packs.
# Deliberately no fallback initials/monograms.
COLORS = {
    "bluetooth": ("#65BAFF", "#2275D2"),
    "appearance": ("#B38CF9", "#7645CA"),
    "notifications": ("#FFB26F", "#E17C30"),
    "apps": ("#80B8F8", "#3F76D1"),
    "account": ("#A48CFA", "#654CC2"),
    "calendar": ("#FFBC8F", "#E36A48"),
    "battery": ("#6DDEB4", "#188A60"),
    "audio": ("#7FBFFF", "#3275C6"),
    "shield": ("#87C5FA", "#2A74BE"),
    "locale": ("#8AE0D7", "#249D9D"),
    "accessibility": ("#84AFFF", "#446EC5"),
    "info": ("#81B9FE", "#397BCB"),
    "grid": ("#A5BAF8", "#637FC6"),
    "keyboard": ("#A7BFD7", "#6783A6"),
    "wifi": ("#7DD1E9", "#2588BE"),
}
NAMES = {
    "bluetooth": "bluetooth", "bluetooth-active": "bluetooth",
    "preferences-desktop-theme": "appearance",
    "preferences-desktop-wallpaper": "appearance",
    "preferences-system-notifications": "notifications",
    "preferences-system-notifications-symbolic": "notifications",
    "applications-other": "apps", "application-x-executable": "apps",
    "avatar-default": "account",
    "x-office-calendar": "calendar",
    "battery": "battery", "battery-full": "battery",
    "audio-card": "audio", "audio-volume-high": "audio",
    "security-high": "shield",
    "preferences-desktop-locale": "locale",
    "preferences-desktop-accessibility": "accessibility",
    "dialog-information": "info",
    "view-grid": "grid",
    "input-keyboard": "keyboard",
    "network-wireless": "wifi",
}
# All icons use a common size, round caps, a color gradient and native vector
# shapes. Unlike the previous generic fallback, no letters are rendered.
SHAPES = {
    "bluetooth": '<path d="M48 18v60l20-19-40-31m20-10 20 19-40 31" />',
    "appearance": '<path d="M48 17C25 17 13 32 13 49c0 20 14 31 32 31 9 0 13-8 9-15-4-8 1-13 9-13h8c9 0 14-7 12-15C79 23 62 17 48 17Z" />'
                  '<circle cx="33" cy="36" r="4"/><circle cx="23" cy="50" r="4"/><circle cx="38" cy="63" r="4"/><circle cx="56" cy="30" r="4"/>',
    "notifications": '<path d="M24 67h48l-8-12V41c0-12-6-20-16-20s-16 8-16 20v14Z" />'
                     '<path d="M41 76c1 6 13 6 14 0" />',
    "apps": '<rect x="17" y="17" width="26" height="26" rx="6"/>'
            '<rect x="53" y="17" width="26" height="26" rx="6"/>'
            '<rect x="17" y="53" width="26" height="26" rx="6"/>'
            '<rect x="53" y="53" width="26" height="26" rx="6"/>',
    "account": '<circle cx="48" cy="32" r="16"/>'
               '<path d="M20 79c0-19 12-29 28-29s28 10 28 29Z"/>',
    "calendar": '<rect x="17" y="23" width="62" height="58" rx="7"/>'
                '<path d="M17 40h62M33 16v16M63 16v16M31 53h11M54 53h11M31 66h11"/>',
    "battery": '<rect x="14" y="30" width="63" height="38" rx="8"/>'
               '<path d="M81 41h3v16h-3M45 35 33 52h12l-5 12 23-23H49l4-6Z"/>',
    "audio": '<path d="M14 40h18l19-17v50L32 56H14Z"/>'
             '<path d="M62 37c13 9 13 22 0 30M70 27c21 14 21 32 0 46"/>',
    "shield": '<path d="M48 14 75 26v21c0 19-12 30-27 38C33 77 21 66 21 47V26Z"/>'
              '<path d="m35 49 9 9 17-19"/>',
    "locale": '<circle cx="48" cy="48" r="32"/>'
              '<path d="M16 48h64M48 16c-19 18-19 46 0 64M48 16c19 18 19 46 0 64"/>',
    "accessibility": '<circle cx="48" cy="18" r="8"/>'
                     '<path d="M17 39h62M48 39v21M48 60 33 81M48 60l15 21"/>',
    "info": '<circle cx="48" cy="48" r="31"/>'
            '<path d="M48 42v22M48 30v1"/>',
    "grid": '<rect x="17" y="17" width="27" height="27" rx="5"/>'
            '<rect x="52" y="17" width="27" height="27" rx="5"/>'
            '<rect x="17" y="52" width="27" height="27" rx="5"/>'
            '<rect x="52" y="52" width="27" height="27" rx="5"/>',
    "keyboard": '<rect x="10" y="26" width="76" height="47" rx="8"/>'
                '<path d="M21 39h7m10 0h7m10 0h7m10 0h5M21 50h7m10 0h7m10 0h7m10 0h5M30 61h38"/>',
    "wifi": '<path d="M12 36c22-20 50-20 72 0M24 48c15-14 33-14 48 0M36 60c8-7 16-7 24 0"/>'
            '<circle cx="48" cy="73" r="4"/>',
}

def artwork(name: str, category: str, variant: str = "light", size: int = 96) -> str | None:
    if category not in ("apps", "actions", "categories", "devices", "panel"):
        return None
    kind = NAMES.get(name)
    if kind is None:
        return None
    first, last = COLORS[kind]
    strokes = {"bluetooth", "audio", "locale", "accessibility", "info", "keyboard", "wifi"}
    stroke = '#F8FBFF' if kind in strokes else '#FFFFFF'
    glyph = SHAPES[kind]
    # Uniform colored surface with one clean white glyph, not app-name initials.
    # Smaller icons stay vector and preserve crisp strokes.
    background = f'<rect x="9" y="9" width="78" height="78" rx="20" fill="url(#tone)"/>'
    graphic = f'<g fill="none" stroke="{stroke}" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round">{glyph}</g>'
    # Replace all strokes by a complete readable motif; shapes are not fonts.
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">'
            '<defs><linearGradient id="tone" x1="0" x2=".7" y1="0" y2="1">'
            f'<stop stop-color="{first}"/><stop offset="1" stop-color="{last}"/>'
            '</linearGradient></defs>'
            + background + graphic + '</svg>')
