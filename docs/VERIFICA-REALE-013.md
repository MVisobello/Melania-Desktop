# Melania v0.13 — rapporto di verifica eseguita

Data: 29 ottobre 2026. Contesto: Debian 13, container isolato.

## Eseguito con successo

- Audit di coerenza dei 13 manifest Cargo e relativi percorsi locali: **PASS**.
- 7 controlli statici di regressione e infrastruttura: **7 PASS**.
- 5 test del motore file Python del prototipo: **5 PASS**.
- Bash syntax check degli script: **PASS**.
- Compilazione Python dei tool di test: **PASS**.
- Test GUI X11 su Xvfb 1280×800: **PASS**.
  - Trascinamento reale con il puntatore di `Leggimi.txt` nella cartella `Progetti`.
  - Copia completata con stessi byte dell'originale.
  - File sorgente mantenuto.
  - Toggle demo Wi-Fi azionato e registrato.
  - Screenshot salvato in `reports/melania-preview-test.png`.

## Bloccato / non verificato

- `cargo check` e `cargo test`: **NOT RUN**. Rustc/Cargo non disponibili.
- Build GTK 4 e Layer Shell: **NOT RUN**. SDK e pkg-config metadata assenti.
- Compositore Smithay annidato: **NOT RUN**. Eseguibile mancante.
- Drag & drop tra app Wayland reali: **NOT RUN**.
- Rendering Aero Glass reale, XWayland, sessione login: **NOT RUN**.
- Accesso a static.rust-lang.org / github.com dal container: DNS non disponibile.

## Miglioramenti al test in questa versione

Nella v0.12 lo smoke test controllava socket e IPC, ma poteva superare anche senza client grafici. Ora la procedura:
1. esige `melania-files` compilato;
2. avvia realmente il client GTK nella sessione del compositore;
3. controlla che il compositore restituisca un toplevel relativo a Melania Files;
4. fallisce esplicitamente quando il client non compare.

**Nota:** la comparsa nell'elenco non dimostra ancora il corretto rendering dei pixel e non è un test completo del drag & drop Wayland.

## Come procedere sul PC Linux

```bash
./scripts/diagnose-on-linux.sh
# Se Cargo/GTK sono installati e la compilazione è riuscita:
xvfb-run -a ./scripts/smoke-nested-wayland.sh
```

Esaminare `reports/linux-diagnostics.txt` e `reports/compile-output.log` e correggere i primi errori del compilatore. Non eseguire Melania come desktop di login principale fino a quando il compositore annidato non è collaudato.
