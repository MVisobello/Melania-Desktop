# Melania Desktop 0.16 — Rapporto verifiche

## Test eseguiti qui
- `python3 -m unittest discover -s tests -p 'test_*.py' -v`: **21/21 superati**. Sono controlli Python su struttura e invarianti, **non test compilati Rust**.
- `python3 -m unittest discover -s preview-lab -p 'test_preview.py' -v`: **5/5 superati**, relativi al vecchio banco di prova Python/X11, non all'integrazione Wayland v0.16.
- `bash -n scripts/run-melania-v016-nested.sh scripts/smoke-notifications-dbus.sh`: **superato**.
- Parsing `tomllib` dei 16 manifest Cargo: **superato**.
- `scripts/audit-build.py`: **static audit PASS**, prerequisiti nativi assenti.

## Test non eseguiti
- `cargo check --workspace`: non disponibile (`cargo` e `rustc` assenti).
- `cargo test` per il nuovo NotificationStore e l'IPC: non disponibile.
- `scripts/smoke-notifications-dbus.sh`: **bloccato** (binario notifiche non compilato, `notify-send` assente).
- `scripts/smoke-nested-wayland.sh`: **bloccato** (GTK4, Smithay e Rust non installati).
- Rendering Aero Glass, drag & drop GTK↔Qt e XWayland non collaudati.

## Problemi attualmente noti
- Il servizio notifiche supporta i metodi principali ma non ancora `expire_timeout`, toast, action callbacks o ricerca nello storico.
- Il pannello notifiche usa snapshot JSON atomico su filesystem temporaneo, con refresh ogni 600 ms; non usa ancora segnali/push per refresh.
- Gestione IPC sincrona nella render loop: 40 ms massimo per connessione possono provocare latenza con client non cooperativi. Prima di 1.0 serve un listener asincrono o isolato dal rendering.
- Il codice compositore Smithay non è mai stato compilato in questo ambiente, quindi rimangono possibili incompatibilità API da correggere attraverso gli errori CI.

## Valutazione
**Sorgenti v0.16 preparati, non una release installabile.** Nessun risultato di compilazione o prova Wayland viene dichiarato superato senza esecuzione.
