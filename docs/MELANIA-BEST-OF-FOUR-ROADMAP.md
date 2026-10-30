# Melania Desktop — Best of Cinnamon, XFCE, GNOME and COSMIC
Data di valutazione: 30 ottobre 2026
Stato: decisioni architetturali e milestone; non dichiarare implementate le funzionalità qui pianificate.

## Principio di prodotto

**Cinnamon come comportamento predefinito; XFCE come disciplina delle risorse;
GNOME come metodo di navigazione; COSMIC per i flussi avanzati.**
Melania rimane un DE indipendente Rust, GTK4, Wayland/Smithay, con identità
Aero Glass e shell tradizionale. Non sarà una copia estetica dei quattro DE.

Principi inderogabili:
1. Barra delle applicazioni, menu Melania e icone desktop tradizionali sempre disponibili.
2. Funzioni potenti ma non obbligatorie: niente auto-tiling attivo per default.
3. Ogni toggle nelle Impostazioni deve corrispondere a una funzione effettiva,
   oppure mostrare esplicitamente che è sperimentale/non collegata.
4. Non eseguire comandi amministrativi senza autorizzazione; settings di sistema
   delegati a API Linux/DBus e privilegi Polkit.
5. Le funzioni di shell/compositor non devono mettere a rischio i dati utente.
6. Distinguere una demo Tk da un componente reale Rust/GTK/Wayland.

## Matrice di selezione funzionale

| Sottosistema | Cosa prendiamo | Ispirazione | Decisione |
|---|---|---|---|
| Desktop | icone, click destro, trascinamento sicuro, ordinamento | Cinnamon/Nemo, XFCE | P0 |
| Menu Start | categorie, ricerca istantanea, preferiti, app recenti | Cinnamon + ricerca GNOME/COSMIC | P0 |
| Taskbar | finestre raggruppate, pinnate, tooltip, anteprime, tray | Cinnamon + XFCE | P0 |
| Settings | sezioni ricercabili, impostazioni oneste, backup/reset | GNOME/COSMIC/Cinnamon | P0 |
| Files | schede, rete, ricerca, rinomina multipla, preview, proprietà | Nemo, Thunar, Nautilus | P0-P1 |
| Window management | snap ai bordi, massimizza, minimizza, alt-tab | Cinnamon/XFCE | P0 |
| Workspaces | conteggio fisso e panoramica; dinamici opzionali | GNOME/COSMIC | P0/P1 |
| Quick settings | Wi-Fi, Bluetooth, audio, luminosità, energia | GNOME | P0/P1 |
| Sistema | notifiche D-Bus, sessione, blocco, avvio automatico | XFCE/GNOME | P1 |
| Accessibilità | ingrandimento, alto contrasto, lettore schermo, navigazione tastiera | GNOME | P1 |
| Ricerca globale | applicazioni + file indicizzati + impostazioni | GNOME/COSMIC | P1 |
| Scorciatoie | editor, conflitti, layout, gesti trackpad | GNOME/COSMIC | P1 |
| Finestre avanzate | auto-tiling per workspace, stack a schede, focus hints | COSMIC | P2 |
| Personalizzazione | colore accent, densità, temi e layout predefiniti | COSMIC/Cinnamon | P1-P2 |
| Pannelli/plugin | aggiungi, sposta, ordina componenti | Cinnamon/XFCE | P2 |
| File manager esteso | dispositivi, rete SMB/SFTP, undo operazioni, monitor | Nemo/Thunar/Nautilus | P2 |
| Multischermo | scala, disposizione, DPI, mapping workspace | GNOME/COSMIC | P2 |
| Sessione | logout, shutdown, ripristino finestre, lock screen | XFCE/GNOME | P2 |
| App store / update | frontend PackageKit/Flatpak, senza duplicare backend | COSMIC | P3 |

### Da NON imitare

- Eliminare il pannello tradizionale in favore della sola Activities overview.
- Requisiti grafici pesanti per ogni postazione, anche a basso consumo.
- Plugin eseguiti con i privilegi del compositor: usare processi isolati e API limitate.
- Duplicazione delle impostazioni di NetworkManager, UPower, BlueZ, PipeWire,
  AccountsService o logind: usare le interfacce di sistema.
- Fingere un blur GPU con semplice CSS trasparente.
- Riusare marchi e asset distintivi dei DE sorgente senza controllo delle licenze.

## Revisione della base Melania esistente

- `melania-settings`: sidebar e pagine in GTK4, tema Aero recente;
  le funzioni hardware rimangono delegate ai servizi di sistema.
- `melania-config`: config utente atomica e validata; ora include numero workspace.
- `melania-compositor`: compositor nested Smithay, gestione focus, geometrie,
  finestre e Snap. Necessari ulteriori test hardware/Wayland.
- `melania-panel`: Start, pin, lista finestre, 4 workspace precedenti,
  quick settings, notifiche, calendario. Serve un layout più responsivo.
- `melania-workspaces`: modello reale di assegnazione finestre, numeri fissi;
  workspace dinamici non ancora presenti.
- `melania-files`: già esteso rispetto alla prima versione, ma non
  equivalente a un sostituto completo e collaudato di Nemo/Thunar/Nautilus.
- `melania-notifyd`: notifica freedesktop e snapshot di sessione in sviluppo.
- Aero GPU: GLSL presente ma compositing background blur da completare.

## Milestone 1 — Impostazioni operative (questa PR)

- `workspace_count` persistente, validazione 1..8 e compatibilità con vecchi file.
- `melania-settings`: selettore numerico e conferma, applicazione al successivo
  riavvio di Melania, senza distruggere o spostare finestre aperte.
- `melania-panel`: il numero di pulsanti workspace segue il valore salvato.
- `melania-compositor`: workspace model inizializzato con quel numero.
- Protocollo IPC: validazione 1..8, il compositor rifiuta comunque spazi non
  esistenti tramite il modello `Workspaces`.
- `snap_windows`: il compositor ora rispetta la preferenza al suo avvio.
- Ricerca locale delle categorie nell'app Impostazioni.

Limitazioni M1:
- Cambiamenti a Snap / conteggio workspace NON live; serve riavviare
  l'ambiente Melania; non terminare la sessione dell'host.
- Non esiste ancora UI per gestire workspace dinamici.
- I pulsanti possono occupare troppo spazio su risoluzioni piccole se si
  selezionano 8 workspace; introdurre overflow/compattazione nella M2.

### Verifica di accettazione M1

1. `cargo test -p melania-config -p melania-control -p melania-workspaces`.
2. `cargo check -p melania-settings -p melania-panel -p melania-compositor`
   su Linux con librerie GTK, Wayland e GLES necessarie.
3. Config precedente senza `workspace_count` -> 4.
4. Valori 0, 9, negativi, testo non numerico -> fallback sicuro a 4.
5. Alla successiva sessione: 2 workspace in Settings -> 2 nel compositor
   e nella taskbar; `WORKSPACE 3` rifiutato.
6. Snap disattivato -> trascinamento al bordo senza auto-aggancio.
7. Cambio numero workspace con 2 finestre aperte -> la sessione corrente
   non deve perderle; restart necessario.
8. Controllo visivo su sessione GTK4 reale: niente switch allungati, ricerca
   accessibile da tastiera, impostazioni leggibili su 870x600.
9. Mai dichiarare CI e prova grafica riuscite senza report effettivo.

## Milestone 2 — Productivity foundation

1. Layout adattivo taskbar: overflow, gruppo finestre e indicatori per
   1–8 workspace; garantire fruibilità su display 1024x768.
2. Ricerca unificata nel menu Melania: app .desktop da GIO, file recenti
   e modulo Settings; nessun crawler illimitato o accesso ad altri utenti.
3. Editor preferiti, taskbar e pulsanti app raggruppati; DnD sicuro.
4. UI "Scorciatoie" con mapping reale nel compositor, conflitti e ripristino;
   scorciatoie base: Alt+Tab, Super+E, Super+L, Super+frecce, Ctrl+Alt+frecce.
5. Selettori app predefinite via GIO/XDG; aggiornamento sincronizzato.

## Milestone 3 — Sistema e accessibilità

- Bluetooth/NetworkManager, PipeWire, UPower, logind, colord, DisplayConfig:
  usare API esistenti (D-Bus) con riconoscimento dispositivo non supportato.
- Energia, sospensione, lock screen, monitor e scaling; test multi-monitor.
- Contrasto elevato, font più grandi, scaling, motion reduction, focus visibile,
  screen reader e semantica GTK accessibile.
- Avvio automatico XDG, session restore opt-in, login manager lasciato al sistema.
- Indice delle preferenze cercabili, diagnostica e reset della sola sessione Melania.

## Milestone 4 — Window Management COSMIC

- Auto-tiling OFF per default e attivazione per workspace.
- Stack di finestre come gruppo di tab (con gestione focus e DnD sicuri).
- Snap Assist per scegliere seconda finestra, separato dal file DnD.
- Overlay panoramica in stile GNOME, senza rimuovere la taskbar.
- Workspace dinamici opzionali SOLO dopo test su finestre trasferite,
  chiuse, minimizzate, monitor secondario, sospensione.
- Compositor GLES con backdrop capture e separable blur, frame pacing,
  fallback senza effetto a bassa potenza.
- Prove Wayland-native, XWayland, GPU diverse, screenshot e regressioni.

## Milestone 5 — Estensioni e file manager avanzato

- Componenti pannello isolati fuori processo, manifest versionati,
  permessi espliciti, API limitate, safe mode e recupero dopo crash.
- Desklet volontarie, non processi obbligatori al login.
- Files: preferiti, rinomina multipla con anteprima, undo filesystem,
  operazioni cancellabili, monitor, GVfs e montaggi remoti autenticati.
- Documentazione licenze; contributi originali e compatibili.

## Criteri di rilascio

- Nessuna regressione su clipboard, drag/drop COPY, rinomina, trash,
  focus e interazione delle finestre.
- Core rust test, GTK compile e Smithay nested smoke devono passare.
- Benchmark RAM al login/idle/4 finestre, tempi login, apertura Files,
  frame-time e responsiveness su CPU lenta.
- Livello qualità accessibilità con tastiera senza puntatore.
- Versione pre-alpha fino a prova end-to-end su sessione reale.
- Non pubblicare promesse equivalenza completa con i quattro DE.

## Riferimenti consultati (funzionalità, non codice copiato)

- Linux Mint Cinnamon Developer Guide:
  https://linuxmint-developer-guide.readthedocs.io/en/latest/cinnamon.html
- Cinnamon Spices: https://cinnamon-spices.linuxmint.com/
- XFCE Documentation: https://xfce.readthedocs.io/latest/
- GNOME Help Multitasking: https://help.gnome.org/gnome-help/multitasking.html.it
- GNOME Quick Settings: https://help.gnome.org/gnome-help/shell-introduction.html
- System76 COSMIC guide: https://support.system76.com/support/pop-basics
- Fedora COSMIC Spin: https://fedoraproject.org/spins/cosmic/
