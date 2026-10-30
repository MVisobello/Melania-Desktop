# Melania Desktop 0.8 — M6: elenco delle finestre e comandi della taskbar

## Cosa è stato scritto

`melania-control` è una nuova libreria Rust **senza dipendenze GUI**, più il CLI `melania-winctl`. Definisce un protocollo locale, versionato per questa release: comandi `LIST`, `ACTIVATE ID`, `MINIMIZE ID`, `CLOSE ID`, con righe di risposta `WINDOW` e terminatore `END`. I campi testuali vengono codificati per impedire che newline o tab dei titoli corrompano il protocollo.

Il compositore assegna un identificativo monotono a ogni nuova `xdg_toplevel`; recupera titolo e `app_id` dalle proprietà xdg del client, espone uno snapshot al pannello e gestisce i comandi **solo** attraverso il compositore. `MINIMIZE` rimuove la finestra dalla scena salvandone la posizione, mentre `ACTIVATE` la rimappa e ripristina il focus; `CLOSE` invia la richiesta Wayland di chiusura, senza terminare arbitrariamente il processo client.

La shell GTK interroga il socket ogni 500 ms. Gli elementi del gruppo "finestre aperte" vengono aggiornati solo se lo snapshot è cambiato: pulsante principale = attiva/minimizza, `×` = richiesta di chiusura. Il gruppo rimane distinto dalle applicazioni fissate e non inventa finestre aperte a partire dai processi Linux.

## Sicurezza

- Socket locale `$XDG_RUNTIME_DIR/melania-<pid>-control.sock`, con permessi `0600` (richiede directory runtime privata);
- Nessuna API esposta via rete e nessun accesso ad altre sessioni;
- Comandi limitati a finestra e identificativo validi, timeout I/O e massimo 128 KiB per risposta;
- Nessuna esecuzione di comandi della shell attraverso IPC;
- Rimozione del socket all'uscita normale del compositore.

**Limitazioni:** la connessione locale non usa un meccanismo di autenticazione aggiuntivo: tutti i processi del medesimo utente che conoscono il percorso possono inviare comandi. È sufficiente per una prova locale, non per un ambiente multiutente o con isolamento forte.

## Drag & drop

Il DnD di `melania-files` e `wl_data_device` rimane separato dal trascinamento della geometria delle finestre. Il pannello contiene ora un `DropTarget` dedicato ai file: dopo 500 ms di hover su una finestra aperta invia al compositore `ACTIVATE ID`. **Non effettua una copia, non sposta né consuma il file**: il rilascio va effettuato nella finestra dell'applicazione, che dovrà negoziare autonomamente i formati MIME supportati. Il mantenimento della sessione DnD durante il cambio di focus e la compatibilità GTK/Qt/XWayland sono ancora da collaudare end-to-end.

## Prova su Linux

1. `./scripts/check-melania-v08.sh` — esegue test e compilazione;
2. `./scripts/run-melania-v08.sh` — avvia la sessione annidata, senza sostituire Cinnamon;
3. In un altro terminale: `./scripts/melania-winctl.sh list` e `./scripts/melania-winctl.sh minimize 1`, poi `activate 1`.

**Non eseguito qui:** mancano toolchain Rust e SDK GTK 4. Le operazioni sopra sono istruzioni per il collaudo e non un risultato già ottenuto.

## Priorità M7

Completare e testare con GPU reale il compositore, sincronizzazione dei popup, `xdg_activation_v1`, input touchpad e il drag dei file sulla taskbar. Integrare XWayland, più monitor e session management prima di qualsiasi dichiarazione di stabilità.
