## 0.19.0 — Desktop interattivo (sorgenti, non collaudato)

- Nuovo `melania-desktop-core` indipendente da GTK per selezione, ordinamento limitato in memoria e cartelle atomiche.
- Desktop GTK: clic singolo seleziona, doppio clic apre, Ctrl+clic seleziona più file.
- Trascinamento di selezioni fino a 256 file, DROP COPY sulle icone delle cartelle.
- Menu contestuale: Nuova cartella, Aggiorna, Melania Files, Impostazioni.
- Corrette scorciatoie localizzate Documenti/Immagini/Download.
- Pipeline Rust aggiornata; 41 test statici + 5 test del prototipo superati.
- **Non** compilati Rust/GTK/Wayland: milestone pre-alpha, nessuna build installabile.

# Melania Desktop — Changelog

## 0.18.0 — Desktop Surface (pre-alpha, non compilata)
- Nuovo modulo Rust `melania-desktop-surface`, client di background Layer Shell.
- Wallpaper incluso e supporto `MELANIA_WALLPAPER`.
- Icone della Scrivania XDG con monitor dei cambiamenti.
- Apertura dei file tramite applicazioni di sistema.
- Drag & drop COPY-only fra il desktop e altre applicazioni che supportano `gdk::FileList`.
- Integrazione degli script e dei job Linux per compilare il nuovo modulo.
- Test di regressione strutturali e documentazione della superficie desktop.

**Nota:** nessuna compilazione Rust e nessun avvio Wayland certificato per questa versione. Il banco prova Tkinter precedente è distinto.

# Melania Desktop 0.17 — Centro notifiche e shell

- Aggiunto `melania-notifyctl` per leggere i contatori, segnare come lette e chiudere notifiche.
- Aggiunta API D-Bus privata `org.melaniadesktop.NotificationCenter1` senza cambiare quella standard freedesktop.
- Aggiunto stato `unread` e limiti byte UTF-8 sui campi delle notifiche; nuovi test Rust per limiti, retrocompatibilità e stato di lettura.
- Aggiunti badge dinamico nella taskbar, pulsante «Segna lette», chiusura per notifica e stile dedicato.
- Migliorata l'esclusività visiva dei popup Start/Quick/Notifiche.
- Esteso lo smoke test D-Bus: ora controlla ricezione, badge, lettura e chiusura; aggiornati pipeline e runner annidato.
- Nessuna nuova dichiarazione di funzionamento Wayland: compilation/test Rust ancora bloccati localmente.

## 0.16.0 — Servizio notifiche e IPC robusto

- Nuovi crate `melania-notify-core` e `melania-notifyd` con servizio notifiche standard D-Bus e snapshot di sessione atomico.
- `melania-panel`: pannello notifiche in basso a destra, coerente con il concept blu Aero approvato.
- `melania-control` e `melania-compositor`: comandi sul socket locale letti fino a EOF prima di autorizzare qualunque operazione, limiti dimensionali e test sui messaggi frammentati.
- Nuova pipeline CI per il servizio D-Bus con test reale via `notify-send` su bus separato.
- Il drag & drop di Melania Files resta COPY-only: nessuna cancellazione automatica dell'origine.
- **Compilazione ed esecuzione Rust/GTK/Wayland non ancora verificate.**

## Versione 0.15

Vedere [CHANGELOG-015.md](CHANGELOG-015.md) per le modifiche ai sorgenti e le verifiche mancanti.

## 0.14 — Melania Files e compatibilità UI
- Nuovo crate melania-files-core, con test su ordinamento, filtri, preferiti, limiti e symlink.
- Schede GTK e buffer interno di copia/taglia condiviso, sorting e preferiti nella sidebar.
- Creazione cartelle, rinomina sicura senza sovrascrittura, estratti di testo in anteprima.
- Nuovi controlli CI e gate che fallisce se mancano Cargo e SDK.
- La compilazione Rust/GTK/Smithay è ancora da eseguire su Linux: nessuna affermazione di build funzionante.

## v0.13 — verification hardening
- Added real GTK client requirement in nested Wayland smoke test.
- CI compositor job now compiles Melania Files and installs GTK4 SDK.
- Added safe one-command Linux diagnostic script.
- Added regression tests against false-positive headless compositor checks.
- Preserved the approved visual identity and existing COPY-only file drag/drop protections.

# Changelog

## v0.12 — Consolidamento visivo e sicurezza DnD
- Eliminazione della cancellazione del sorgente al termine del drag COPY-only in Melania Files.
- Filtro rapido Foto nel file manager.
- Focus visivo per comandi shell e file manager; CSS migliorato.
- Specifica di test e limitazioni aggiornata.

# Melania Desktop — Change log

## 0.11.0 — UI implementation pass
- Refactor del pannello Start GTK a due colonne e collegamenti rapidi.
- Pannello rapido e taskbar con stile più fedele al concept approvato.
- Preview delle immagini nell'area dettagli di Melania Files.
- CSS coordinato della suite GTK4 e specifica UI aggiornata.
- Nessuna dichiarazione di compilazione o Wayland funzionante in assenza della toolchain.

# Changelog

## 0.10.0 — UX Validation Lab (2026-10-29)

- Conservati i 13 crate Rust esistenti della v0.9; portata la versione di workspace a 0.10.0 **senza affermare che compilino**.
- Aggiunto `preview-lab/melania_preview.py`: prototipo grafico realmente avviabile sotto Linux/X11/XWayland, distinto dal compositore.
- Interfaccia dimostrativa: desktop, taskbar, Start, ricerca, quick settings, Files, Impostazioni, finestre spostabili e Aero Snap sperimentale.
- Implementato e provato drag & drop con il mouse da file a cartella; copia locale sicura che non sovrascrive e non rimuove la sorgente.
- Cinque test unitari Python superati; smoke test GUI Xvfb verificato con screenshot.
- Aggiunta pipeline CI (compilazione Rust + test di UX separati) e documentazione dei limiti reali.
- Restano NON VERIFICATI: compilazione Rust/GTK4, avvio compositor Smithay, layer shell, Qt/XWayland cross-app DnD.

## 0.9.0 — Codice sorgente sperimentale ereditato

Vedi i documenti storici `docs/COMPOSITOR-M7.md` e `docs/ROADMAP.md`.

## Build validation branch (after v0.12)
- Added offline dependency/manifest audit with JSON report.
- Added distinct core / GTK / compositor Rust compilation gates and CI log artifacts.
- Added nested Wayland socket + control IPC smoke script, intended to run only after successful compilation.
- Preserved X11 UI prototype tests but labeled them correctly as non-Wayland.
- Documented exact limitations and acceptance criteria for the first verified Wayland session.

## v0.20 — Melania Aero Icons (30 ottobre 2026)
- Nuovo tema freedesktop `Melania`: 96 icone principali, 13 symbolic, export PNG 16–256 px.
- Riutilizzo del logo ufficiale a mezzaluna con punto; nuove icone originali per il desktop.
- Integrazione icon theme nei sorgenti di Panel, Files, Settings e Desktop Surface.
- Sidebar di Melania Files con icone, e riconoscimento delle cartelle italiane per le icone speciali.
- Launcher della taskbar e controlli rapidi con icone del tema.
- Installer utente non invasivo e test automatici.
- Nessuna modifica alle policy COPY-only del drag & drop.
