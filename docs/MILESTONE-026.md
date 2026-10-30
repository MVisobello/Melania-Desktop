# Melania Desktop 0.26 — gestione degli spazi di lavoro e preparazione Aero Glass

## Implementato nei sorgenti
- Libreria std-only `melania-workspaces` per assegnazione delle finestre a 4 spazi di lavoro, con unit test Rust.
- Nuovi comandi IPC locali e limitati: `STATUS`, `WORKSPACE N`, `SEND ID N`.
- Mappatura/smappatura delle finestre del compositore quando si cambia spazio di lavoro, senza alterare `minimized` e senza copiare/spostare file.
- Quando l'utente seleziona dalla taskbar una finestra di un altro spazio, Melania passa a quello spazio prima di attivarla.
- Selettore compatto 1–4 sulla taskbar GTK che legge lo stato dal compositore.
- Funzione geometrica testabile per clamp popup rispetto ai confini del monitor, ancora da collegare al protocollo xdg-positioner.
- Sorgenti GLSL per sfocatura separabile e compositing con tinta Aero. **NON ancora integrati nella pipeline Smithay/GLES**.

## Non completato
- Compilazione Rust/GTK/Smithay e avvio Wayland reale: mancanti Cargo/GTK 4 in questo ambiente; nessuna build verificata.
- Blur effettivo: per funzionare il renderer deve campionare il framebuffer sotto la superficie, eseguire due pass FBO e comporre il risultato, con attenzione a danni e scissor.
- Anteprime fotografiche della taskbar: richiedono un percorso sicuro di cattura delle superfici, caching e invalidazione.
- Popup XDG: la funzione di contenimento è scritta, ma la gestione runtime dei popup/grab e del posizionamento deve essere integrata.
- XWayland, multimonitor e drag MOVE multiprocesso: richiedono ulteriori implementazioni e test.

## Test su Linux reale
```bash
cargo test -p melania-workspaces -p melania-control
cargo check -p melania-panel -p melania-compositor
cargo run -p melania-control --bin melania-winctl -- status
```
Con una sessione Melania annidata, il comando CLI accetta anche `workspace N` e `send ID N` (N da 1 a 4).
