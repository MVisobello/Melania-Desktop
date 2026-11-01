# Melania Files — Anteprima testo GIO/GVfs

La scheda file di rete può mostrare un'**anteprima rapida di sola lettura** tramite **Anteprima testo** o **F3**. Il file non viene eseguito, non viene aperto in un browser e non viene modificato: i caratteri vengono visualizzati come testo semplice in un `GtkTextView` non modificabile.

## Sicurezza

- L'anteprima è **esplicitamente richiesta dall'utente** e rifiuta cartelle, file speciali e collegamenti simbolici (`NOFOLLOW_SYMLINKS`).
- Sono ammessi tipi MIME testuali e alcuni equivalenti strutturati (JSON/XML/YAML). I file con byte NUL vengono rifiutati. Una codifica diversa da UTF-8 può mostrare caratteri sostitutivi.
- La lettura è limitata a **64 KiB + 1 byte** per riconoscere quando il file è più grande; il contenuto completo di un file remoto non viene scaricato.
- Ogni chiamata GIO riceve il `Gio::Cancellable` del backend; chiudere la finestra richiede l'annullamento. La risposta effettiva dipende dal provider.
- L'interfaccia non renderizza HTML o script, né esegue comandi, macro o programmi.
- Test Rust reali su file locali verificano contenuto, limite byte, rifiuto del binario e annullamento prima della lettura. La prova SFTP su server temporaneo in GitHub Actions legge inoltre un file di testo attraverso il vero provider GVfs.

## Limiti

Non sono ancora presenti anteprime immagine, video, PDF o miniature di sistema. La funzione è una base sicura verso Quick Look, **non** un sistema completo di anteprime equivalente a Nautilus. Vedere issue #78.
