# Rapporto di verifica — Melania v0.6

**Ambiente disponibile:** container Debian 13 senza `rustc`, `cargo` e SDK GTK 4. Un tentativo di aggiornamento dei pacchetti ha raggiunto il timeout; **non è stato possibile compilare il workspace né aprire una sessione Wayland**.

## Controlli eseguiti

- Controlli Python (`scripts/static-check.py`) dei manifest TOML, dei percorsi dipendenze, dei formati SVG/PNG e degli script Shell.
- `bash -n` sugli script, controllo struttura ZIP e verifica dell'identità grafica.
- API confrontate con documentazione pubblica Smithay 0.7 (move_request, resize_request, Space::relocate_element, Window::geometry, ToplevelState).

## Controlli ancora necessari

1. `./scripts/check-melania-v06.sh` su Linux con Rust stable (>=1.87), GTK4 SDK e librerie GL.
2. `./scripts/run-melania-v06.sh` in desktop Wayland o X11 host.
3. Interazione manuale: clic, movimento finestra, ridimensionamento, popup, rilascio del tasto.
4. File usa-e-getta con DnD fra due finestre Files, crash, cancel, collisioni e più file.
5. Qt, XWayland, Flatpak e accessibilità richiedono implementazioni e test ulteriori.

Le verifiche statiche non garantiscono la compilazione o il comportamento runtime.
