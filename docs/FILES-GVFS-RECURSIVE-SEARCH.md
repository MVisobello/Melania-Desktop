# Ricerca ricorsiva GIO/GVfs — Melania Files

Le schede GVfs offrono **Cerca nelle sottocartelle**, separato dal filtro rapido della cartella corrente. L'utente scrive almeno due caratteri nel filtro e avvia volontariamente la ricerca. I risultati comprendono nomi e URI cliccabili, con avanzamento sul numero di elementi esaminati.

### Sicurezza e limiti
- Ricerca **solo per nome**, non per contenuto.
- Massimo **8 livelli** di sottocartelle, **300 cartelle visitate**, **5.000 elementi esaminati** e **300 risultati**; nessun ciclo tramite symlink (GIO NOFOLLOW_SYMLINKS).
- L'annullamento usa il cancellabile nativo del provider. Se la cartella iniziale non è accessibile, l'errore viene mostrato senza fingere risultati.
- Le sottocartelle inaccessibili vengono contate e omesse; il numero è mostrato, anziché dichiarare complete le ricerche incomplete.
- Non sono richiesti né conservati file, contenuti o password. La ricerca non modifica dati remoti.
- Il tempo di risposta dipende dal backend GVfs e dal server. Non è un servizio di indicizzazione con cache persistente e **non è ancora parità completa con Nautilus**.

I test Rust usano cartelle reali `file:///`, nomi nascosti e symlink; la CI GTK/Wayland collauda l'integrazione del desktop. I provider SMB/SFTP/WebDAV reali restano da collaudare separatamente.
