# Melania 0.12 — Implementazione UI e file manager

## Lavoro realmente eseguito nei sorgenti Rust/GTK
- DragSource in Melania Files: pubblicizza solo COPY e non cancella mai file originari nel callback di fine trascinamento.
- Filtro immagini rapido `Foto` nel file manager: limita la lista a cartelle e immagini supportate.
- Aggiornamenti GTK CSS per taskbar, file manager, impostazioni, inclusi focus visibili per tastiera.
- Restano invariati compositor e protocolli esistenti, non ancora verificati in runtime.

## Test richiesti in ambiente Linux
1. `cargo check --workspace`
2. `cargo test --workspace`
3. Avvio `melania-files` su display GTK
4. Trascinamento file su cartella: verifica copia e integrità originale.
5. Filtro Foto e ricerca contemporaneamente.

## Limitazioni
- Il drag & drop MOVE inter-applicazione non è ancora attivo.
- Il compositore non è stato compilato in questa sessione.
- Alcuni effetti Aero richiedono implementazione nel compositore, non possono essere ottenuti con CSS.
