# Melania 0.16 — Notifiche, shell e IPC

## Moduli aggiunti
- `melania-notify-core`: coda Rust con massimo 100 messaggi, limiti al testo, sostituzione per ID, rimozione, snapshot JSON atomico nell'area privata `XDG_RUNTIME_DIR/melania/notifications.json`.
- `melania-notifyd`: servizio `org.freedesktop.Notifications` su D-Bus utente con metodi Notify, CloseNotification, GetCapabilities e GetServerInformation e segnale NotificationClosed per le chiusure esplicite. Le applicazioni inviano via protocollo freedesktop, senza integrazioni proprietarie.
- `melania-panel`: pannello notifiche GTK4 Layer Shell in basso a destra, aggiornato dallo snapshot del demone; mostra titolo, applicazione e testo, senza eseguire alcun contenuto remoto.

## Architettura notifiche

```text
Applicazione Linux → session D-Bus → melania-notifyd → Store Rust
                                                     ↓ snapshot atomico (0600)
melania-panel (GTK/Layer Shell) ← file JSON privato ←┘
```

Per evitare conflitti con GNOME, KDE o Cinnamon, **non** eseguire il servizio notifiche nella sessione principale finché Melania non è pronta. Lo script `run-melania-v016-nested.sh` crea un bus D-Bus **isolato**.

## IPC compositor ↔ taskbar

I comandi `LIST`, `ACTIVATE id`, `MINIMIZE id` e `CLOSE id` viaggiano su un socket Unix con permessi 0600. Il client conclude la scrittura con `shutdown(Write)`, il compositore legge fino a EOF con un massimo di 65 byte: un messaggio incompleto, multiplo o eccessivo non attiva alcuna finestra.

## Test che devono essere realmente eseguiti

Su un Linux completo:

```bash
cargo check -p melania-control -p melania-notify-core -p melania-notifyd -p melania-panel
cargo test -p melania-control -p melania-notify-core
cargo build -p melania-notifyd
./scripts/smoke-notifications-dbus.sh
./scripts/run-melania-v016-nested.sh
```

Lo smoke D-Bus chiama `notify-send` su un bus isolato e verifica che lo storico JSON contenga il messaggio.

## Limitazioni e requisiti ancora aperti

- **Non compilato in questo ambiente**: Cargo e gli header GTK4 non sono disponibili qui. Il codice necessita dei test CI per confermare le API.
- Il server non gestisce ancora la scadenza temporizzata delle notifiche, la loro visualizzazione toast, le icone remote e le azioni cliccabili; il testo viene conservato in uno storico temporaneo fino al termine della sessione.
- «Non disturbare» è una preferenza salvata, ma il sistema non mostra ancora toast da sopprimere.
- XWayland e i portali Flatpak non sono stati integrati nel compositore.
- Il drag & drop fra applicazioni GTK/Qt, la selezione degli appunti Wayland, il rendering Aero Glass e i popup complessi non sono ancora collaudati end-to-end.
- Il compositore resta annidato e sperimentale: non installarlo come sessione predefinita.

## Perché questi componenti

Le notifiche, i layer e i canali di controllo separati rispettano i confini fra il compositore (finestre/input), la shell (UI) e i servizi di sessione (D-Bus/clipboard). Il codice sorgente del file manager non è stato sostituito: permangono schede, preview, copia prudenziale e drag & drop COPY-only.
