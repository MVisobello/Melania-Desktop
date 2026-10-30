# Melania v0.19 — rapporto controlli, 30 ottobre 2026

| Test | Risultato | Attendibilità |
|---|---|---|
| `python3 -m unittest discover -s tests` | **41/41 PASS** | statica, struttura sorgenti |
| `python3 -m unittest discover -s preview-lab -p test_preview.py` | **5/5 PASS** | funzionale, ma esclusivamente vecchio prototipo Python/X11 |
| `bash -n` script v0.18 e v0.19 | **PASS** | sintassi script |
| `cargo test -p melania-desktop-core` | **NON ESEGUITO** | Cargo assente |
| `cargo check -p melania-desktop-surface -p melania-panel` | **NON ESEGUITO** | Cargo/GTK non disponibili |
| Avvio compositore Smithay | **NON ESEGUITO** | build non disponibile |
| Drag & drop GTK4 interprocesso | **NON VERIFICATO** | richiede sessione Wayland reale |

Lo script `scripts/check-melania-v019.sh` è stato eseguito e ha prodotto **exit code 3**:
controlli Python superati, compilazione Rust non eseguita. Non esiste ancora
una prova di funzionalità GTK della nuova selezione, del menu contestuale o
del drop su cartella. Le API GTK utilizzate vanno compilate e collaudate prima
di usarle con dati importanti.
