# Melania Desktop 0.17 — Centro notifiche utilizzabile

## Componenti coinvolti

- `melania-notify-core`: storico limitato a 100 notifiche, identificativi stabili, campo `unread` retrocompatibile, limite dei testi in **byte UTF-8**; modifica della generazione solo quando lo stato cambia.
- `melania-notifyd`: mantiene il protocollo standard `org.freedesktop.Notifications` e offre una API privata `org.melaniadesktop.NotificationCenter1` sul path `/org/melaniadesktop/NotificationCenter` dello stesso servizio D-Bus.
- `melania-notifyctl`: client CLI per il nuovo protocollo privato e la chiusura standard delle notifiche.
- `melania-panel`: visualizza il contatore delle notifiche non lette nella taskbar, pulsante «Segna lette» e pulsante «×» per ogni notifica; mantiene l'estetica traslucida blu.

## Comandi

```bash
melania-notifyctl unread
melania-notifyctl mark-read
melania-notifyctl close 12
```

Le operazioni hanno semantica distinta:

- `mark-read` modifica soltanto lo stato di lettura; **non** chiude la notifica e non emette `NotificationClosed`.
- `close ID` invoca `CloseNotification` standard, elimina l'elemento dallo storico corrente e il servizio emette `NotificationClosed` con motivo `3` (richiesta API).
- Una sostituzione `Notify(replaces_id)` aggiorna lo stesso ID e lo rende nuovamente non letto.

Il demone crea uno snapshot JSON limitato per utente in `$XDG_RUNTIME_DIR/melania/notifications.json`. L'interfaccia lo legge periodicamente per evitare di bloccare il thread GTK con chiamate D-Bus. Non archivia notifiche permanenti oltre la sessione.

## Limiti intenzionali

- Nessun toast temporizzato, azioni D-Bus, icone delle applicazioni, o timer di scadenza: sono prossime milestone e richiedono la corretta emissione del segnale `NotificationClosed`.
- «Non disturbare» è ancora una preferenza locale senza soppressione toast: non viene presentato come funzione completa.
- Le chiamate GTK e zbus non sono state compilate in questo ambiente.
- Il drag & drop in Melania Files resta **COPY-only** fino al collaudo end-to-end di MOVE Wayland.
