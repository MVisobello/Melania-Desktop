# Melania 0.18 — Rapporto di collaudo

Data: 30 ottobre 2026.

## Eseguito in questa sessione
- `./scripts/check-v018-offline.sh`: **PASS**, 34 test Python (`unittest`).
- `bash -n` degli script di avvio: **PASS**.
- Struttura dei moduli Cargo, integrazione della pipeline e file .desktop: **PASS** ai test statici.
- Wallpaper originale: PNG 1600×900 con intestazione valida.
- Test di verifica asset e collegamenti fra moduli: **PASS**.

## Non verificato (rilascio bloccato)
- Toolchain Rust (`cargo`, `rustc`) assente.
- GTK 4 di sviluppo e gtk4-layer-shell assenti.
- Non è stato possibile raggiungere i repository esterni per installare la toolchain.
- Non sono stati eseguiti `cargo check`, test Rust o una sessione Wayland.
- Non sono stati provati drag & drop GTK/Qt/XWayland o Aero blur reali.

I 34 test verificano **invarianti dei sorgenti**, non il funzionamento del codice Rust.

## Comandi successivi su Linux attrezzato

```bash
cargo check -p melania-desktop-surface
cargo check --workspace
cargo test -p melania-transfer -p melania-shell-model -p melania-dnd
./scripts/run-melania-v018-nested.sh
```

Non installare questa build come desktop principale.
