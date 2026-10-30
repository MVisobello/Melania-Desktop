# Melania Fluent — icon pack for Melania Desktop

The three icon variants are original illustrations **inspired by the Windows 11 / Fluent visual language**: compact colored special folders, approachable shapes, 2–3 color ramps, restrained shadows, geometric monochrome toolbar and system icons. These are **not Microsoft icons** and do not include proprietary Windows resources. The Flat Remix coverage is used for file and icon identifiers only — no Flat Remix artwork is copied.

## Themes

- `Melania-Fluent-Light`
- `Melania-Fluent-Dark`
- `Melania-Fluent-Light-on-Dark-Panel`

All themes are freedesktop `index.theme` icon themes. Install **all three together** to preserve cross-theme relative symlinks. From an extracted package directory:

```bash
bash INSTALLA-MELANIA-FLUENT.sh
```

No host desktop preferences are changed. Use Melania Settings → Appearance to activate the preferred theme. This release is a *standalone theme*: the pre-existing Melania Aero themes are preserved.

## Coverage and art completeness

The package inherits the full icon-name inventory already catalogued in Melania plus explicit 16/24/32/48/64/96/128/256 SVG paths for the selected main icons. `BUILD-REPORT.json` records verified path resolution. **Technical coverage does not mean each alias is a separately hand-drawn illustration.** The main icons have dedicated artwork; unknown app identifiers receive differentiated fallback icons and other names use semantic family artwork. These should be refined further in future releases.

## Build from sources

From the Melania Desktop repository root:

```bash
python3 tools/build_melania_fluent_themes.py --output dist/melania-fluent-icons
```

Or for an offline build against an exported existing Melania archive:

```bash
python3 tools/build_melania_fluent_themes.py --reference-tar old-Melania-Aero-Complete.tar.xz --output dist/melania-fluent-icons
```

The builder never reads the SVG/PNG content in a reference archive; it reads filenames only.

## Included sizes

SVG icons can be scaled to any size. For core icons the theme also provides explicit 16, 24, 32, 48, 64, 96, 128, and 256 pixel paths and a simplified small-size treatment. System symbolic icons are distinct in Light and Dark.