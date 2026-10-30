# Report di verifica v0.7 — 29 ottobre 2026

## Eseguito nell'ambiente di generazione
- Parsing di tutti i manifest `Cargo.toml` con Python `tomllib`.
- Verifica della presenza di tutti gli undici crate del workspace e relativi path.
- Controllo sintattico Bash (`bash -n`) di tutti gli script `.sh`.
- Parsing XML degli SVG e integrità PNG con Pillow.
- Controllo statico di simboli per DnD, layer-shell, Aero Snap e test unitari predisposti.
- Verifica integrità dello ZIP e SHA-256 finale.

## Non eseguito
- `cargo check --workspace` / `cargo test`: cargo e rustc assenti.
- Compilazione GTK e Layer Shell: non sono installate le librerie di sviluppo native.
- Test GTK/Qt/XWayland, rendering/drag & drop, focus dei popup o integrazione dispositivi: nessuna sessione Linux grafica con librerie necessarie disponibile.

L'esito positivo dei controlli statici **non prova la compilabilità o il funzionamento**. I sorgenti vanno testati e corretti su una macchina Linux prima di usarli.
