# Melania Desktop — audit autonomo 31 ottobre 2026

## Stato di integrazione

Questa fase è nel ramo `feature/start-identity-safe-session-20261031` e nella PR #22. **Non considerare il lavoro validato o pronto a `main` senza compilazione reale e test.**

### Implementato nel codice

- Start: rimozione dell'intestazione artificiale, ricerca in focus, `Esc` per la chiusura, filtro azzerato alla chiusura; pulsante orb circolare con hover e stato aperto. Restano invariati i pulsanti applicazione rettangolari.
- Logo: il simbolo canonico proviene da `branding/icons/melania-symbol.svg` in ogni generatore di temi iconografici Melania esaminato, comprese le varianti Fluent e Aero; niente ricostruzione inventata della mezzaluna.
- Azioni sessione: comandi separati Spegni/Riavvia/Iberna/Sospendi/Disconnetti/Blocca, controllo supporto e conferma esplicita; lo spegnimento host è **vietato** per le preview annidate. Il blocco è disabilitato finché non esisterà un lockscreen effettivo.
- App recenti: annotazione soltanto di avvii riusciti, tramite ID dell'applicazione; massimo 8 ID su `$XDG_STATE_HOME/melania/recent-applications`, 4 accessi mostrati nel menu. Nessuna telemetria.
- Impostazioni Privacy: cancellazione del file di cronologia delle app recenti, senza eliminare directory o cronologie di altri desktop.
- Impostazioni Schermo: rilevamento e refresh dell'accelerometro mediante `iio-sensor-proxy`. Rotazione automatica/manuale ancora **non supportata**, esplicitamente indicata.

### Verifiche

- Commit e presenza dei sorgenti verificati attraverso GitHub.
- Controlli statici su struttura, riferimenti all'SVG e protezioni delle azioni: **11 passati su 11** durante la sessione; questi NON compilano Rust o GTK.
- CI attuale: i job di `main` e PR #22 terminano con `failure` senza step eseguiti. I log non sono disponibili tramite l'integrazione GitHub. Possibile blocco di infrastruttura, runner o quota, **causa non determinata**.
- Test Rust/GTK reali: **non eseguiti**.
- Screenshot reali del pannello aggiornato: **non acquisiti**.
- Nessun merge su `main`.

### Verifiche indispensabili, quando un runner Linux torna disponibile

```bash
python3 -m unittest discover -s tests -p 'test_*.py' -v
cargo check --workspace --all-targets
cargo test --workspace
cargo build -p melania-compositor -p melania-panel -p melania-files -p melania-settings
# Solo in un display Linux di prova con sessione annidata:
./scripts/smoke-full-desktop-wayland.sh
```

I test hardware per lockscreen, DRM/KMS, sospensione e rotazione non devono eseguire azioni distruttive sulla sessione host. Conservare gli screenshot generati dalla pipeline e ispezionarli prima del merge.

## Domanda di riesame — cosa manca per renderlo migliore?

1. **P0 — Ripristino CI**: issue #23. Senza job eseguiti nessuna modifica grafica o funzionale va dichiarata pronta.
2. **P0 — Sessione nativa**: issue #24. Il compositore `melania-compositor` è ancora annidato; servono DRM/KMS, libseat/logind, login e gestione sicura delle sessioni.
3. **P1 — Rotazione e input**: issue #25. Occorre applicare gli output transform in Smithay, sincronizzare il touch e verificare il comportamento multimonitor.
4. **P1 — Preferiti e pinned app generici**: l'implementazione attuale della barra supporta soltanto i due launcher Melania; la lista delle applicazioni installate è reale ma non gestisce ancora preferiti per tutte le app.
5. **P1 — Ricerca file indicizzata**: la ricerca Start copre app, categorie delle Impostazioni e cartelle XDG note, **non l'intero contenuto dei documenti**; serve un indicizzatore/portal senza scansioni bloccanti nel thread GTK.
6. **P1 — Impostazioni hardware**: connettere i controlli ai servizi e al compositore, senza interferire con un desktop ospitante.
7. **P2 — UX reale**: collaudo Light/Dark/Aero, resizing, tastiera, screen reader, HiDPI e confronto degli screenshot effettivi per regressioni.

Questo documento elenca le lacune deliberatamente: nessuna finta funzionalità, nessuna falsa verifica.
