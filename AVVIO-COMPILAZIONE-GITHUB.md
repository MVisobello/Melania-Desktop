# Melania 0.38 — Preparazione compilazione Linux reale

## Risultato in questa sessione
La toolchain Rust e le dipendenze GTK4/Wayland **non sono installate** nell’ambiente corrente. `apt-get update` fallisce con `Temporary failure resolving deb.debian.org`. Nessuna build è stata eseguita e non c’è alcun eseguibile compilato.

## Correzioni della pipeline
- Job Rust GTK e compositore su `ubuntu-26.04`, dove è disponibile `libgtk4-layer-shell-dev`.
- Aggiunta installazione GTK4 Layer Shell anche al job del compositore.
- Aggiunto `set -o pipefail` dove l’output è salvato con `tee`, così gli errori del compilatore non diventano successi apparenti.
- Il job del compositore ora tenta di compilare anche `melania-panel` e `melania-settings`.
- Solo in caso di build riuscita carica gli eseguibili nell’artifact `melania-linux-x86_64-binaries`.
- Il test dell’avvio Wayland rimane separato e potrà fallire anche se i binari compilano.

## Avvio della CI
1. Pubblicare il contenuto della cartella `melania-desktop` nella radice di un repository GitHub (inclusa `.github/workflows/linux-verify.yml`).
2. Aprire **Actions** e selezionare **Melania Linux build gates** > **Run workflow** oppure fare un push.
3. Controllare gli esiti separati: `rust-core`, `rust-gtk`, `rust-notifications`, `rust-compositor`.
4. Scaricare eventuali artifact di log o binari. In caso di errori Rust inviare i log per applicare le correzioni.

Un artifact binario compilato non equivale a una sessione grafica totalmente collaudata.
