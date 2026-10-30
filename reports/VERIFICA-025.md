# Melania Desktop — Rapporto verifica 0.25

Data: 30 ottobre 2026.

## Ambiente disponibile
- La toolchain Rust/Cargo **non è presente**.
- La compilazione di `melania-panel` / `melania-compositor` e una sessione Smithay live **non sono state eseguite**.
- L'archivio non va installato come desktop principale.

## Test realmente effettuati
- `python3 scripts/check-v025-static.py`: **28/28 superati**, compresi 12 confronti del calcolo mensile con `datetime` Python. Controlli statici del sorgente e regole del modello (non compilazione).
- `python3 -m unittest discover -s tests -p 'test_*.py' -q`: **41 test Python superati**; verificano invarianti preesistenti e struttura.
- `python3 tools/test_icon_theme.py`: **8 test superati** sul tema SVG/PNG e metadati.
- `bash -n scripts/verify-v025-shell.sh` e `bash -n scripts/run-melania-v018-nested.sh`: **sintassi valida**.
- `./scripts/verify-v025-shell.sh`: **bloccato come atteso**, exit 3 con `BLOCCATO: cargo non installato`.

## Cosa NON è stato testato
- Compilazione `cargo check`.
- I test Rust definiti nei moduli `melania-shell-model` e `melania-control`.
- Gestione XDG delle finestre durante un uso Wayland reale.
- Render/blur trasparente e drag-and-drop Qt/GTK/XWayland.
- Screenshot reali dell'interfaccia Rust/Wayland modificata.

## Prossimo gate
1. `./scripts/verify-v025-shell.sh` su Linux con toolchain e librerie.
2. Correzione degli eventuali errori del compilatore (in particolare API GTK4 e Smithay).
3. Avvio annidato, test menu Start / calendario / controllo finestre, cattura dello screenshot reale.
4. Solo dopo: popup XDG completi, cattura anteprime taskbar, sfocatura Aero GL.
