# Melania Desktop — stato verificato al 1° novembre 2026

Melania Desktop è un ambiente desktop sperimentale per Linux, basato su
**Rust, GTK 4 e Smithay/Wayland**. Il repository contiene componenti reali
(compilati in Linux CI) e un compositore che funziona in **modalità annidata**
dentro un sistema grafico già avviato. **Non è ancora un desktop nativo
installabile come sessione di login**: non è presente un backend DRM/KMS e
libseat/logind completo.

## Verifiche e interfaccia attuale

- GitHub Actions compila i componenti GTK e Smithay, esegue test Python/Rust,
  prova D-Bus e una sessione Wayland annidata con vere finestre e screenshot.
- Menu Start ispirato a Windows 7, ricerca pronta alla digitazione, tasto Super,
  ricerca e alimentazione allineati nel footer, tre temi Light/Dark/Aero.
- Taskbar con pin di applicazioni freedesktop installate (clic destro nel menu
  Start o sulla barra), riordino e persistenza locale degli ID validati.
- Desktop virtuali selezionabili da un unico pulsante; orologio stile Windows
  con **ora sopra la data**, entrambe allineate al medesimo margine destro.
- Melania Files: le schede locali e quelle GVfs utilizzano ora la
  **stessa enumerazione e gli stessi metadati GIO** (PR #81), con ordinamento,
  filtri e cancellazione delle letture condivisi. Le schede remote supportano
  anche selezione multipla, Ctrl+A, copia e trascinamento multipli.
  Creazione esclusiva di file/cartelle, rinomina e Cestino sono instradati
  attraverso il backend comune (PR #82); su Linux la rinomina locale conserva
  la garanzia atomica anti-sovrascrittura. I volumi GVfs montati sono accessibili
  dalla barra laterale. PR #83 aggiunge la connessione autenticata tramite
  GtkMountOperation e i controlli Monta/Smonta/Espelli non forzati con conferma;
  PR #84 mostra byte reali ed elementi completati durante le copie GVfs.
  PR #85 aggiunge ordinamento Nome/Data/Dimensione/Tipo nelle schede di
  rete, file nascosti (Ctrl+H), proprietà GIO asincrone e monitoraggio
  delle cartelle quando supportato dai provider. PR #86 limita a una
  per scheda le letture GVfs attive, scartando risultati obsoleti e
  mantenendo soltanto l'ultima richiesta in coda.
  PR #87 collega la cancellazione direttamente alle chiamate native Gio;
  PR #88 aggiunge ricerca ricorsiva remota per nome, annullabile e con limiti;
  PR #89 esegue un vero test di integrazione SFTP autenticato su un server
  temporaneo GitHub Actions (lettura, metadati, copia e anti-sovrascrittura).
  PR #90 introduce Annulla/Ripeti per una rinomina per scheda, con controllo
  d'identità del file, nessuna sovrascrittura intenzionale, Ctrl+Z e
  Ctrl+Maiusc+Z; ora la CI esegue anche i test reali del backend Melania Files.
  PR #91 aggiunge Anteprima testo/F3 per file locali e remoti via GIO/GVfs,
  in sola lettura e limitata a 64 KiB; provata anche via SFTP autenticato.
  PR #92 aggiunge la stessa anteprima F3 per PNG/JPEG/WebP (6 MiB massimo,
  dimensioni e pixel limitati, decodifica in worker Rust), collaudata anche
  con lettura e verifica dei pixel su un vero server SFTP temporaneo.
  PR #93 consente a F3 di riconoscere le immagini anche quando un provider
  GVfs non comunica il MIME nell'elenco, verificando comunque il formato
  effettivo dei dati prima di mostrare l'anteprima.
  Build Rust/GTK, Wayland e integrazione desktop passati.
  Restano adapter PathBuf per viste locali avanzate e funzioni POSIX,
  test SMB/WebDAV, dialoghi di password reali e dispositivi fisici, oltre
  a undo/redo completo per copie e spostamenti, miniature di directory,
  anteprime PDF/video, decoding immagini isolato e proprietà avanzate.
  La parità Nautilus **non è ancora completa**.
  Dettagli: [backend GIO/GVfs](docs/FILES-GIO-GVFS-URI-BACKEND.md),
  [autenticazione/mount](docs/FILES-GVFS-MOUNT-AUTH.md),
  [vista remota avanzata](docs/FILES-GVFS-RICH-VIEW.md),
  [worker GVfs](docs/FILES-GVFS-WORKER-LIMITS.md),
  [annullamento nativo](docs/FILES-GVFS-NATIVE-CANCELLATION.md),
  [ricerca ricorsiva](docs/FILES-GVFS-RECURSIVE-SEARCH.md) e
  [test SFTP autenticato](docs/FILES-GVFS-SFTP-LOOPBACK-TEST.md) e
  [Annulla/Ripeti rinomina](docs/FILES-GVFS-RENAME-UNDO.md) e
  [Anteprima testo GIO/GVfs](docs/FILES-GVFS-TEXT-QUICKLOOK.md) e
  [Anteprima immagini GIO/GVfs](docs/FILES-GVFS-IMAGE-QUICKLOOK.md).

## Avvio e sicurezza

La prova annidata può essere avviata da un Linux grafico esistente:

```sh
bash scripts/melania-session --nested
```

Per aggiungerla facoltativamente al menu applicazioni **dell'utente**:

```sh
bash scripts/install-melania-preview-launcher.sh
```

Questo **non** installa una sessione in `wayland-sessions`, non modifica il
display manager e non richiede privilegi di amministratore. I controlli per
un avvio nativo rispondono esplicitamente che il backend manca:
`bash scripts/melania-session --check-native`.

Roadmap tecnica: [Sessione nativa](docs/NATIVE-SESSION-ROADMAP.md);
problemi aperti: [#24 sessione autonoma](https://github.com/MVisobello/Melania-Desktop/issues/24),
[#26 pin taskbar](https://github.com/MVisobello/Melania-Desktop/issues/26) e
[#27 ricerca file Start](https://github.com/MVisobello/Melania-Desktop/issues/27).

*Le note storiche qui sotto riportano lo stato delle versioni precedenti e
possono contenere avvertenze ormai superate dai test CI più recenti.*

---

# Melania Desktop 0.36 — aggiornamento Files

**Pre-alpha, Rust/GTK 4 non ancora compilato in questa sessione.** Il progetto continua dalla v0.35, senza modifiche all’identità Aero o alle icone ufficiali.

Vedi `../README-v036.md` per le modifiche e i limiti di verifica.

---

# Melania Desktop 0.25 — Shell & Compositor (pre-alpha)

Il codice di GTK Shell e Smithay è stato aggiornato secondo le immagini approvate del menu Start e del desktop completo. **Non ancora compilato o testato su una sessione Wayland reale.**

- [Documentazione delle modifiche](docs/SHELL-COMPOSITOR-025.md)
- [Changelog 0.25](CHANGELOG-025.md)
- Controlli offline: `python3 scripts/check-v025-static.py`
- Test compilazione Linux: `./scripts/verify-v025-shell.sh` (richiede Cargo, GTK4 e layer shell)
- Avvio annidato sperimentale: `./scripts/run-melania-v018-nested.sh`

---

## Melania 0.21 — Icon Pack Aero Classic v2

Tema basato sulle icone classiche approvate, integrato nel codice GTK. [Dettagli](docs/ICONE-021.md). I sorgenti Rust/Wayland restano da compilare.

---

# Melania Desktop — v0.19 pre-alpha (30 ottobre 2026)

**Novità di questa iterazione (sorgenti Rust/GTK 4):** clic singolo per selezionare,
doppio clic per aprire, Ctrl+clic per selezione multipla, drag di più file, drop
COPY sulle icone delle cartelle, menu contestuale sul desktop e creazione di
cartelle senza sovrascrittura. Nuova libreria Rust `melania-desktop-core` e
scorciatoie XDG del menu Start corrette per le directory in italiano.

**Stato:** 41 controlli strutturali Python e 5 test del prototipo passati;
**Rust, GTK 4 e Smithay non compilati e non collaudati** in questa sessione.
Non installare come desktop principale. La grafica obiettivo resta il concept
blu Aero approvato con logo a mezzaluna e punto.

```bash
./scripts/check-melania-v019.sh
```

In assenza di Cargo, lo script termina con codice **3**, senza segnalare una build
falsamente riuscita. Consulta [le note della v0.19](docs/DESKTOP-INTERAZIONI-019.md)
e [il rapporto di verifica](reports/VERIFICA-019.md).

---

# Melania Desktop — v0.18 pre-alpha

Melania è un desktop environment Linux indipendente, sviluppato in **Rust + GTK 4 + Wayland/Smithay**, con estetica ispirata a Windows 7 Aero e compatibilità applicativa come obiettivo di progetto simile a Cinnamon.

**Stato reale:** i moduli Rust/GTK sono sorgenti sperimentali; questa versione **non è compilata né installabile** in modo certificato. Il prototipo Tkinter/X11 rimane un banco di prova separato e non rappresenta l'interfaccia finale.

## Novità della v0.18

Abbiamo aggiunto una prima **superficie desktop Wayland vera a livello di sorgenti** (`melania-desktop-surface`), distinta dalla demo grafica precedente. È ancorata al layer `Background` sotto le finestre, legge la cartella Scrivania XDG, disegna le icone, apre i file e consente copie tramite drag & drop GDK (COPY-only), con monitor automatico dei cambiamenti.

La taskbar e il menu Start conservano il design Aero blu approvato. Il drag & drop non deve mai perdere i file originali: MOVE rimane disabilitato fino ai test end-to-end.

## Prova annidata (NON eseguire come desktop principale)

Dopo aver installato Rust, GTK 4, gtk4-layer-shell e le dipendenze Smithay:

```bash
cargo check -p melania-desktop-surface
./scripts/run-melania-v018-nested.sh
```

Selezione wallpaper locale:

```bash
MELANIA_WALLPAPER="$HOME/Immagini/sfondo.png" ./scripts/run-melania-v018-nested.sh
```

La prova va eseguita **dentro una sessione Linux già funzionante**; lo script crea una finestra annidata e non sostituisce la tua sessione. Potrebbero emergere errori di compilazione delle API GTK/Smithay ancora da risolvere.

## Audit senza Rust

```bash
./scripts/check-v018-offline.sh
```

Questa procedura verifica i file, gli script e la connessione dei moduli. **Non equivale a una compilazione Rust.**

## Documentazione

- [Nuovo desktop e DnD](docs/DESKTOP-SURFACE-018.md)
- [Direzione grafica approvata](docs/UI-SPEC-APPROVATA.md)
- [Roadmap](docs/ROADMAP.md)

---

> **Melania 0.17:** nuova gestione delle notifiche lette/non lette, contatore taskbar, chiusura per ID e `melania-notifyctl`. L'API D-Bus rimane compatibile con `org.freedesktop.Notifications`. I sorgenti NON sono ancora compilati e verificati con Rust/GTK. Vedi [documentazione 0.17](docs/NOTIFICHE-017.md) e [rapporto test](reports/VERIFICA-017.md).

# Melania Desktop 0.17 — Notifiche e integrazione shell/compositore (pre-alpha)

**Sorgenti incrementali dalla v0.15.** Questa versione aggiunge un servizio notifiche freedesktop, il centro notifiche GTK Layer Shell, test di regressione D-Bus e una lettura IPC più sicura per i comandi delle finestre. **Non è ancora una release compilata e collaudata del compositore Rust/Wayland.**

Leggi [la documentazione completa della 0.16](docs/NOTIFICHE-E-INTEGRAZIONE-016.md). Per il test senza sostituire il tuo desktop, usa `scripts/run-melania-v016-nested.sh` dopo l'installazione delle dipendenze Linux documentate sotto.

# Melania Desktop — 0.15, pre-alpha non compilata

**Nuovo in 0.15:** integrazione iniziale degli appunti GTK di sistema (COPY), Cestino recuperabile GIO, icone native, cartelle XDG localizzate. Il drag & drop rimane COPY-only. Nessuna build Rust/GTK/Wayland è stata verificata in questa sessione; non installare come desktop principale.

- [Implementazione appunti e Cestino](docs/INTEGRAZIONE-CLIPBOARD-TRASH-015.md)
- [Test realmente eseguiti](reports/VERIFICA-015.md)
- [Novità](CHANGELOG-015.md)

# Melania Desktop — v0.15 (sorgenti pre-alpha)

La versione 0.15 continua il progetto di desktop environment indipendente in **Rust, GTK 4 e Wayland/Smithay**, con la grafica blu Aero approvata dall'utente. Non sostituisce ancora Cinnamon, GNOME o Plasma.

## Novità 0.15
- Melania Files: **schede**, preferiti persistenti, ordinamento nome/data/dimensione, creazione cartelle, rinomina senza sovrascrittura, anteprima di testi brevi.
- Nuova libreria `melania-files-core` senza dipendenze grafiche con test unitari.
- Copia/Taglia/Incolla interna condivisa tra le schede; DnD esterno **COPY only** per evitare cancellazioni premature.
- CI e script di verifica aggiornati. Per requisiti tecnici, dettagli e limiti leggi [IMPLEMENTAZIONE-014.md](docs/IMPLEMENTAZIONE-014.md).

## Verifica su Linux
```sh
./scripts/check-melania-v014.sh
```
Il comando verifica struttura, test Rust e componenti GTK/Smithay, ma **richiede Cargo, GTK 4 e librerie Wayland installate**. Senza di esse termina con errore, non simula una build completata.

## Stato
Il codice è stato controllato staticamente in questo ambiente. **Nessuna compilazione Rust/GTK/Smithay né avvio Wayland sono stati verificati qui.**

# Melania Desktop — v0.13 compilation gate (pre-alpha)

**Status:** source + diagnostic checks. Rust/GTK4/Smithay build not yet verified. No installation into system sessions is attempted.

New in this version: CI and smoke test require *an actual GTK client window registered by the compositor*, not merely a live Wayland socket. Start here: `docs/BUILD-GATE-013.md` and `./scripts/diagnose-on-linux.sh`.

# Melania Desktop — build validation package (v0.12 source)

> **Status:** static checks and legacy X11 preview tested. The Rust/GTK4/Smithay source has NOT been compiled in this environment. This is a verification-ready source bundle, not an installable desktop release.

See [Build validation and Linux instructions](docs/BUILD-VALIDATION.md).

# Melania Desktop v0.12 — pre-alpha

Sorgenti Rust/GTK4 + Smithay non ancora verificati in compilazione in questa sessione.
La v0.12 aggiunge un filtro Foto, migliora gli stili e rende il trascinamento COPY-only conservativo: nessuna cancellazione del file sorgente.

Per il progetto e le dipendenze Linux, consultare le istruzioni seguenti.

# Melania Desktop 0.11 — UI implementation pass

**Sorgenti Rust + GTK4 + Smithay** derivati dalla v0.10, con implementazione estetica allineata al concept approvato. Questa release **non è ancora compilata/validata su Wayland**; la demo Tkinter è soltanto il banco di prova UX della v0.10 e non è stata usata come sostituto del codice GTK.

## Novità
- Start GTK4 a due colonne, con accessi rapidi e ricerca applicazioni.
- Stile GTK dei pannelli, taskbar, file manager e Impostazioni in blu Aero / superfici chiare.
- Anteprima immagini reale nell'inspector di Melania Files.
- Drag & drop COPY-only preservato per sicurezza.

## Per compilare su Linux
Seguire `scripts/bootstrap-debian-dev.sh`, poi `cargo check --workspace` e `cargo test --workspace`. Per le prove annidate consultare `scripts/run-melania-v09.sh` e la documentazione del compositor. Trattare gli script come sperimentali e **non sostituire la sessione desktop principale**.

Dettagli: `docs/UI-SPEC-APPROVATA.md`.

# Melania Desktop — 0.10 UX Verification Lab

**Rust + GTK4 + Wayland/Smithay** è l'architettura del DE definitivo. Il logo ufficiale è una mezzaluna blu aperta a destra con un punto interno, come nel concept fornito.

## Stato verificato

✅ **Primo prototipo grafico interattivo realmente avviato** su Xvfb/X11.

✅ Menu Start, finestra Files, Impostazioni, taskbar e centro rapido visibili; drag & drop di un file in una cartella reale del laboratorio con controllo di integrità; cinque test Python del motore di copia superati.

⚠️ **Questa NON è ancora la sessione Rust/GTK4/Wayland richiesta come milestone finale.** Nessun test cargo o prova Smithay è stato completato: in questo ambiente non erano installati Rust/Cargo e GTK4 SDK e il download esterno era impossibile. Non è corretto presentare i moduli Rust ereditati dalla v0.9 come compilati o funzionanti.

### Come provare SUBITO la sessione grafica funzionante

```bash
python3 preview-lab/melania_preview.py
```

**Dipendenze per il prototipo:** Linux/X11 o XWayland, Python 3 con `tkinter` e Pillow. Usando il prototipo non si modifica la distribuzione Linux né la sua schermata di accesso. I controlli del centro rapido sono dimostrativi.

### Test automatici riproducibili

```bash
python3 preview-lab/test_preview.py
./scripts/test-preview-lab.sh /tmp/melania-preview.png
```

### Sviluppare e tentare di avviare il vero compositore Rust/Wayland

```bash
./scripts/bootstrap-debian-dev.sh --install
cargo check --workspace --all-targets
cargo test --workspace
./scripts/run-melania-v09.sh
```

**Attenzione:** i sorgenti Rust sono pre-alpha e la compilazione non è verificata. Gli ultimi comandi non sono garantiti riuscire; eseguirli solo su un Linux di prova, non sostituire Cinnamon. Una pipeline CI è in `.github/workflows/linux-verify.yml` per rendere verificabili errori e test.

Per tutti i risultati e i limiti consultare **[docs/VERIFICA-V010.md](docs/VERIFICA-V010.md)**; per istruzioni grafiche leggere **[preview-lab/README.md](preview-lab/README.md)**.

Il drag & drop completo fra GTK4, Qt e XWayland e una vera sessione Smithay rimangono traguardi da raggiungere.

## Melania Aero Icons — v0.20

È stato aggiunto il **tema icone ufficiale Melania Aero**: icone vettoriali per cartelle, applicazioni, documenti, periferiche, stato e azioni, con PNG da 16 a 256 px. Il tema segue il design Aero blu approvato e usa il logo a mezzaluna già scelto.

Anteprima: `branding/melania-icons-banner.png`  ·  Catalogo: `branding/anteprima-melania-icons.png`

Installazione locale, senza cambiare Cinnamon:

```bash
./scripts/install-melania-icons.sh
```

La variante `--activate` è facoltativa e modifica il tema GTK corrente solo quando viene richiesta esplicitamente. Le applicazioni GTK di Melania sono state aggiornate per richiamare le icone per nome.

Documentazione: [docs/ICONE-MELANIA-020.md](docs/ICONE-MELANIA-020.md). Questa release grafica **non indica** che il compositor Rust/Wayland sia stato compilato o collaudato.


## Novità 0.26 — spazi di lavoro e preparazione Aero
La shell include un selettore per quattro spazi di lavoro, gestito via IPC dal compositore. Il core Rust delle assegnazioni e le funzioni di contenimento popup sono isolati e testabili. Sono inclusi gli shader GLSL preparatori per la sfocatura Aero Glass, ma la sfocatura GPU non è ancora integrata. Vedi `docs/MILESTONE-026.md`.
