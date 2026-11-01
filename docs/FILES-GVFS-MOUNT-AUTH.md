# Melania Files: GVfs interactive authentication and safe volume lifecycle (1 Nov 2026)

## Aggiunte
- Schede GVfs: comando **Connetti** esplicito, implementato con `gio::File::mount_enclosing_volume` e `gtk::MountOperation`. GTK/GVfs presenta i propri prompt per password e autenticazione: Melania non copia, legge, registra né persiste tali credenziali.
- Nessun mount automatico quando l'utente digita un URI; il listing mostra gli errori e il nuovo tasto Connetti serve per autorizzare volontariamente l'operazione. **Annulla operazione** invia la cancellazione GIO.
- Sidebar VolumeMonitor: volumi rimovibili/partizioni non montati sono visibili e mostrano **Monta**, sempre con eventuale prompt nativo.
- Volumi già montati, locali o remoti, espongono **Smonta** ed **Espelli** solo quando il provider GIO ne dichiara la possibilità. Ogni azione richiede ulteriore conferma.
- Nessun uso di `FORCE`; le operazioni in conflitto con file in uso falliscono e mostrano l'errore. Le origini e i dati utente non vengono cancellati.
- La sidebar si aggiorna ai segnali GIO volume_added/removed/changed, oltre a mount_added/removed.

## Requisiti e limiti
Il sistema deve avere i backend GVfs appropriati e la sessione utente D-Bus attiva. Un server SMB/SFTP/WebDAV non è simulato dall'utente: testare credenziali e connessione con endpoint reali in ambiente controllato è un'ulteriore attività.

Il codice implementa il flusso di connessione e gestione volumi, ma i test annidati CI verificano build/GUI, non l'interazione contro server autenticati o dispositivi USB fisici. Non dichiarare la parità Nautilus raggiunta. I mount/unmount non devono essere invocati automaticamente nei test e non vengono eseguiti sulla macchina dell'utente da ChatGPT.

Issue #78 resta aperta.
