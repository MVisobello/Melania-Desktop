#!/usr/bin/env python3
"""Make an actual raster QA sheet of the three installed SVG icon themes.

This renders the built artifacts; it never substitutes stock imagery.
Requires Pillow and CairoSVG (CI installs these in a throwaway venv).
"""
from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw, ImageFont

THEMES = ("Melania-Light", "Melania-Dark", "Melania-Light-on-Dark-Panel")
SAMPLES = (
    ("Cartella", "places/scalable/folder.svg"),
    ("Documenti", "places/scalable/folder-documents.svg"),
    ("Home", "places/scalable/user-home.svg"),
    ("Cestino", "places/scalable/user-trash.svg"),
    ("Server", "places/scalable/network-server.svg"),
    ("PDF", "mimetypes/scalable/application-pdf.svg"),
    ("Testo", "mimetypes/scalable/text-x-generic.svg"),
    ("Computer", "apps/scalable/computer.svg"),
    ("Internet", "apps/scalable/applications-internet.svg"),
    ("Copia", "actions/24/edit-copy.svg"),
    ("Taglia", "actions/24/edit-cut.svg"),
    ("Aggiorna", "actions/24/view-refresh.svg"),
    ("Wi-Fi", "panel/network-wireless-signal-excellent.svg"),
    ("Audio muto", "panel/audio-volume-muted-symbolic.svg"),
)
SIZES = (16, 24, 48, 96)


def _font(size: int):
    name = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    try:
        return ImageFont.truetype(name, size)
    except OSError:
        return ImageFont.load_default()


def render(root: Path, destination: Path) -> None:
    width, row_height, top = 1800, 120, 146
    height = top + len(SAMPLES) * row_height + 44
    img = Image.new("RGB", (width, height), "#e7edf4")
    d = ImageDraw.Draw(img)
    title_font, head_font, label_font = _font(31), _font(17), _font(18)
    d.rectangle((0, 0, width, 112), fill="#102a47")
    d.text((30, 21), "MELANIA  /  AERO ICON QUALITY SHEET", font=title_font, fill="#f3faff")
    d.text((32, 72), "Vettori reali del progetto  ·  anteprima a 16 / 24 / 48 / 96 px",
           font=head_font, fill="#a7c9e6")

    theme_x = (220, 745, 1270)
    for i, theme in enumerate(THEMES):
        x = theme_x[i]
        d.text((x + 16, 121), theme.replace("Melania-", "").replace("-", " "),
               font=head_font, fill="#163a59")
    for row_idx, (label, relative) in enumerate(SAMPLES):
        y = top + row_idx * row_height
        d.text((20, y + 45), label, font=label_font, fill="#1e3950")
        for theme_idx, theme in enumerate(THEMES):
            x = theme_x[theme_idx]
            # Hybrid shows both light body and dark panel, the rest use
            # their native surface; toolbar and status glyphs need contrast.
            panel = relative.startswith("panel/")
            is_dark = theme_idx == 1 or (theme_idx == 2 and panel)
            bg = "#183047" if is_dark else "#f9fbfe"
            fg = "#f4f9ff" if is_dark else "#28425b"
            d.rounded_rectangle((x, y + 3, x + 504, y + 113),
                                radius=13, fill=bg)
            source = root / theme / relative
            if not source.is_file():
                raise FileNotFoundError(source)
            offsets = (22, 106, 217, 348)
            for size, offset in zip(SIZES, offsets):
                svg = source.read_bytes()
                png = cairosvg.svg2png(bytestring=svg, output_width=size,
                                       output_height=size)
                icon = Image.open(BytesIO(png)).convert("RGBA")
                if icon.getbbox() is None:
                    raise ValueError(f"Rendered icon is blank: {source} at {size}px")
                ix = x + offset + (96 - size) // 2
                iy = y + 9 + (96 - size) // 2
                img.paste(icon, (ix, iy), icon)
                d.text((x + offset + 33, y + 94), str(size),
                       font=_font(11), fill=fg)
    destination.parent.mkdir(parents=True, exist_ok=True)
    img.save(destination, optimize=True)
    print(f"Saved actual SVG render QA: {destination}, {img.size}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path,
                        default=Path("dist/melania-icon-themes"))
    parser.add_argument("--png", type=Path,
                        default=Path("dist/Melania-Aero-Visual-QA.png"))
    args = parser.parse_args()
    render(args.output, args.png)
