# Melania Desktop 0.17 — Rapporto di verifica

Data: 30 ottobre 2026. Stato: sorgenti sperimentali, non rilasciabili come DE completo.

## Controlli eseguiti in questo ambiente

- **PASS** — 27 test `unittest` Python sulle invarianti strutturali (compresi 6 nuovi controlli v0.17).
- **PASS** — 5 test di trasferimento file del prototipo X11.
- **PASS** — Smoke GUI su Xvfb, risoluzione 1280×800: drag del file, copia byte-identica, sorgente intatto, toggle dimostrativo Wi-Fi e screenshot.
- **PASS** — Sintassi Bash degli script di avvio annidato e test notifiche.
- **PASS** — Manifest TOML del workspace e degli asset (verifica nel pacchetto).

## Non verificato / bloccato

- **BLOCKED** — `cargo check`, `cargo test` e compilazione GTK/Smithay: Cargo e Rust non disponibili nell'ambiente corrente, download esterni non risolvibili.
- **BLOCKED** — Test reale `org.freedesktop.Notifications` con demone Melania: richiede il binario Rust compilato.
- **BLOCKED** — Drag & drop GTK ↔ Qt / XWayland nel compositore: necessita di una sessione Melania Wayland avviata.
- **BLOCKED** — Aero Glass con blur reale: non implementato nel rendering del compositore.

**Attenzione:** gli smoke test Xvfb riguardano `preview-lab` (Tk/X11) e non certificano il codice GTK4 né il compositore Rust.

## Verifica consigliata su Linux

```bash
cargo test -p melania-notify-core
cargo check -p melania-notifyd -p melania-notifyctl -p melania-panel
cargo build -p melania-notifyd -p melania-notifyctl
./scripts/smoke-notifications-dbus.sh
```

Lo smoke D-Bus usa `dbus-run-session`, non interferisce con le notifiche del desktop già in uso, verifica ricezione, contatore non lette, «Segna lette» e chiusura singola.
