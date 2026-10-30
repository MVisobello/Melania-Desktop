# Rapporto v0.14 — 29 ottobre 2026

## Eseguiti in questo ambiente
- `python3 scripts/audit-build.py --json reports/offline-audit-v014.json`: PASS, 14/14 crate trovati, dipendenze locali valide.
- `python3 -m unittest discover -s tests -p test_release_invariants.py -v`: PASS, 10 test strutturali.
- `python3 -m unittest discover -s preview-lab -p test_preview.py -v`: PASS, 5 test Python del **prototipo**, non test Rust.
- `bash -n scripts/check-melania-v014.sh`: PASS.
- Controllo lessicale dei delimitatori Rust tramite Pygments: 17 sorgenti, nessuna parentesi sbilanciata; NON equivale a `cargo check`.
- `./scripts/check-melania-v014.sh`: EXIT 30, blocco atteso e segnalato correttamente, poiché Cargo non è installato.

## Non eseguiti
- Compilazione del workspace con `cargo check`.
- Test unitari Rust della libreria `melania-files-core`, `melania-transfer` e altri moduli.
- Rendering GTK, avvio del compositore Wayland e drag & drop tra applicazioni.

## Conclusione
Sorgenti v0.14 pronti per la verifica su Linux, senza affermazioni di funzionamento non comprovate.
