# Melania Desktop 0.8 — Registro verifiche

Questa è una release di **sorgenti pre-alpha**, non una versione funzionante certificata.

| Verifica | Esito |
| --- | --- |
| Presenza codice IPC di finestra, CLI e test unitari | Presente |
| Wire protocol per elenco, attiva, minimizza, chiudi | Implementato nei sorgenti |
| Gestione scena / focus / unmap / map | Implementata nei sorgenti, da verificare |
| Taskbar GTK collegata al socket locale | Implementata nei sorgenti, da verificare |
| Hover del file sulla taskbar → richiesta di attivazione della finestra | Implementato nei sorgenti, non collaudato |
| Drag & drop fra applicazioni | Non collaudato |
| `cargo test` | **NON ESEGUITO**, Rust/Cargo assenti |
| `cargo check --workspace` | **NON ESEGUITO**, Rust/Cargo assenti |
| Test con applicazioni GTK e Qt reali | Non eseguito |
| Test XWayland | Non eseguito |

L'assenza di test compilati significa che **non è possibile garantire che il codice Rust compili senza correzioni**. La priorità di chi riceve il progetto è eseguire `check-melania-v08.sh` su Linux e riportare gli errori, anziché installare il compositore come sessione di sistema.
