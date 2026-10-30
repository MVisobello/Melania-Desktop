# Melania Desktop 0.25 — Shell e compositore

## Direzione grafica (vincolante)
Questa iterazione utilizza le due immagini approvate dall'utente:
- `docs/references/start-menu-approved.png`: menu Start con ricerca, programma+icona, colonna destra, footer.
- `docs/references/shell-approved.png`: desktop completo, taskbar blu vetro, finestre luminose, notifiche, area rapida, calendario e workspace.

Il risultato di riferimento è **Melania Aero Classic**, non il vecchio prototipo Tkinter. Il logo ufficiale a mezzaluna blu rimane invariato. Il pack finale delle icone approvate è mantenuto.

## Implementato nei sorgenti Rust
### Shell (GTK4 + gtk4-layer-shell)
- Menu Start riprogettato: elenco dinamico `gio::AppInfo::all()`, icona **autentica** di ciascuna applicazione con `GIcon`, campo ricerca con stato vuoto, scorciatoie XDG e footer Melania.
- Taskbar: pulsanti finestre con icona ottenuta da `app_id`, mantenendo minimizza/ripristina/chiudi tramite IPC. Non finge anteprime fotografiche che non esistono.
- Centro rapido: nuovo selettore visivo Notifiche/Area rapida, riutilizzando servizi Wi-Fi, Bluetooth, audio e luminosità.
- Calendario: pannello Layer Shell separato, apertura con clic sull'orologio, navigazione per mesi, logica gregoriana in `melania-shell-model` con test unitari.
- Orologio taskbar: ora e data.
- Stile: token blu Aero, effetto vetro simulato con trasparenze GTK, pannelli scuri, icone e selezioni coerenti.
- «Esci da Melania»: comando privato `QUIT` sul socket protetto del compositore, che termina **solo** la sessione annidata e i client avviati da Melania; non esegue shutdown/reboot dell'host.

### Compositore (Smithay)
- Gestione esplicita di `xdg_toplevel.maximize/unmaximize/minimize_request`: area massimizzata rispettando le riserve del layer-shell e ripristino della geometria precedente.
- Riarrangiamento dei layer al commit, quando cambia la loro dimensione o esclusività, oltre che alla creazione/distruzione.
- Autorizzazione ai movimenti/resize solo quando il pulsante è stato premuto **sulla superficie della finestra reale**: i clic sulle finestre coperte da taskbar e overlay non creano seriali validi per un'altra finestra.
- IPC `QUIT` testato a livello di regola di parsing (la compilazione Rust è richiesta per eseguire il test).

## Funzioni ancora NON realizzate o non collaudate
- **Compilazione completa:** in questo ambiente non sono presenti Cargo e lo stack GTK4 di sviluppo. Quindi la versione non va chiamata compilata, installabile o funzionante su Wayland.
- Sfocatura reale Aero Glass: la trasparenza GTK esiste a livello CSS, ma il blur vero richiede uno shader/pipeline di compositing.
- Anteprime vere delle finestre: il compositore fornisce metadati e comandi, non una cattura texture condivisa con la taskbar.
- Gestione completa dei popup XDG e dei grab: ancora da progettare/testare.
- XWayland, Flatpak Portal, multimonitor/HiDPI, DnD MOVE e clipboard cross-toolkit: **non verificati**.
- L'area «Recenti» nella schermata di riferimento non è ancora una cronologia reale: per evitare risultati inventati, in questa versione si mostrano «Accessi rapidi».
- Non si è tentato di sostituire il desktop host, né di spegnere il computer.

## Come verificare su Linux
```
./scripts/verify-v025-shell.sh
```
La compilazione e i test vanno fatti su una distribuzione Linux con Rust/Cargo, GTK4, gtk4-layer-shell e dipendenze Smithay disponibili. Per la prova grafica annidata:
```
./scripts/run-melania-v018-nested.sh
```
Lo script di verifica termina con errore esplicito se mancano le dipendenze e non sostituisce i test grafici.
