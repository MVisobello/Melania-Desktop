# Melania Files — cancellazione nativa delle operazioni GVfs

Questa modifica collega l'annullamento della lettura e della copia ai parametri `Gio::Cancellable` delle chiamate che possono attendere un provider remoto.

- Un unico osservatore temporaneo per enumerazione o copia ricorsiva traduce il flag cancellabile dell'interfaccia GTK in `Gio::Cancellable.cancel()`.
- `enumerate_children`, `next_file`, query di metadati, creazione di cartelle e copia ricevono il cancellabile nativo.
- La funzione che osserva l'annullamento termina quando il lavoro del provider restituisce il controllo: nessun thread di osservazione rimane attivo tra le operazioni.
- Le copie rimangono **non sovrascriventi**, con origine intatta. GVfs può comunque lasciare file di destinazione parziali se un trasferimento viene interrotto: l'interfaccia deve avvisarne l'utente e non dichiarare rollback completo.
- Nessun server esterno viene contattato nei test unitari: un test controlla il segnale di annullamento con un provider simulato, un altro controlla che la cancellazione prima di una copia locale non modifichi origine o destinazione.

**Limiti**: la rapidità con cui un'operazione si interrompe dipende dal backend GVfs e dal server; una richiesta cancellabile non è un timeout garantito. Non è stata verificata l'autenticazione contro server SMB/SFTP/WebDAV reali. La parità Nautilus rimane un obiettivo aperto in #78.
