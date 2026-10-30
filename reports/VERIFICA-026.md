# Rapporto di verifica — Melania Desktop 0.26

## Verifiche effettivamente eseguite in questa sessione

- `python3 scripts/check-v026-offline.py`: **10/10 controlli statici superati**.
- `python3 scripts/check-v025-static.py`: **28/28 controlli statici/di modello superati** (base mantenuta).
- `python3 -m unittest discover -s tests`: **41/41 test Python superati**.
- `python3 -m unittest discover -s preview-lab -p test_preview.py`: **5/5 test del prototipo Python superati**.
- `python3 tools/test_icon_theme.py`: completato con esito **OK**.
- `bash -n scripts/*.sh`: sintassi degli script verificata.
- `python3 scripts/audit-build.py --json reports/offline-audit-v026.json`: **20/20 crate presenti**.
- Pesi del filtro Gaussian GLSL: somma **0.999999** (verifica aritmetica); shader non compilati via GPU.

## Verifiche che NON sono state eseguite

- `cargo test` / `cargo check`: **non eseguiti**, Rust/Cargo non disponibili.
- Compilazione GTK 4 e Smithay: **non eseguita**, librerie native non disponibili.
- Test di input Wayland, cambio desktop virtuale e DnD su GUI reale: **non eseguiti**.
- Blur Aero reale e anteprime taskbar: **non attivi**.
- Esecuzione XWayland / multi-monitor: **non implementati o non collaudati**.

I test Python verificano invarianti di struttura, non dimostrano la correttezza o compilabilità del codice Rust.
