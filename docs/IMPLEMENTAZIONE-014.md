# Melania Desktop 0.14 — implementazione reale e limiti

## Obiettivo
Sviluppare funzionalità concrete in Rust/GTK 4 senza modificare la direzione grafica approvata. I moduli qui elencati sono **codice sorgente**, non una build collaudata.

## Funzioni implementate nei sorgenti

### Melania Files
- Schede GTK4 indipendenti; pulsanti **Nuova scheda** / **Chiudi scheda**. Una chiusura non elimina l'ultima scheda.
- Copia/Taglia/Incolla nel **buffer interno condiviso** tra le schede (non ancora appunti globali Wayland).
- Directory listing tramite nuova libreria Rust `melania-files-core` (separata dalla UI): ordinamento per nome/data/dimensione, inversione, filtri, limite massimo per listing.
- Preferiti persistenti in `$XDG_CONFIG_HOME/melania/bookmarks`; formati con codifica esadecimale dei nomi POSIX, supporto anche a nomi non UTF-8. Aggiunta/rimozione e navigazione dalla sidebar.
- Creazione cartella con nomi validati e senza sostituzione di destinazioni esistenti.
- Rinomina senza sovrascrittura su Linux (`renameat2` + `RENAME_NOREPLACE`), con test di regressione.
- Anteprima laterale per immagini e snippet di testo (max 2 KiB per file); i contenuti non vengono eseguiti.
- Il drag & drop esterno continua ad annunciare **COPY only**: non cancella la sorgente. Lo spostamento può essere eseguito con Taglia/Incolla in un filesystem che supporti la rinomina atomica.

### Test e CI
- Nuovi test Rust in `melania-files-core` e `melania-transfer`.
- Aggiornata `.github/workflows/linux-verify.yml` per verificare il modulo aggiunto.
- Controlli strutturali Python estesi; `scripts/check-melania-v014.sh` **fallisce** se manca Cargo.

## Limitazioni esplicite
- **Non compilato** in questa sessione: mancano Cargo/Rust/GTK4. Nessun risultato di test Rust viene dato come superato.
- Non è ancora una sessione Wayland installabile o testata; il compositore Smithay non è ancora convalidato.
- Le cartelle con moltissimi file possono bloccare temporaneamente la UI: in questa fase l'IO del file manager è sincrono.
- Nessuna anteprima taskbar generata dal compositore; non c'è ancora blur Aero autentico.
- Drag & drop completo inter-app, MOVE negoziato, XWayland, screenshot, clipboard esterno, multi-monitor, portals, notifiche complete, tastiera/accessibilità e packaging restano attività future.

## Gate per la fase successiva
Eseguire `./scripts/check-melania-v014.sh` su Linux con toolchain e SDK installati. Correggere **gli errori effettivi di compilazione** prima di ampliare il codice; verificare la sessione annidata con client GTK e Qt reali.
