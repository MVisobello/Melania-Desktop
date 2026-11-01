# Melania Files — GIO/GVfs URI backend (1 novembre 2026)

## Cosa è stato implementato
- Nuove **schede di rete** nella stessa barra di Melania Files (UI e temi originali): pulsante Rete, `network:///`, apertura di URI con Ctrl+L dalla barra locale o remoto. `melania-files sftp://utente@server/cartella` è accettato da linea di comando.
- Enumerazione GIO `gio::File`, senza conversione in `PathBuf`, per risorse con backend GVfs installato: `smb://`, `sftp://`, `dav://`, `davs://`, `ftp://`, `mtp://`, `gphoto2://`, `afp://`, `nfs://`, `network:///` e `file:///`.
- Listing su worker thread, risultati limitati (2.000 elementi), nessun accesso remoto sincrono sul thread GTK, filtri, cronologia, cartella superiore, F5, Ctrl+L, Ctrl+F.
- Selezione/apertura GIO, clipboard GDK `FileList`, drag-and-drop COPY e incolla fra altre app, copia file/cartelle locali verso remoto e remoto verso locale, creazione cartella, rinomina e Cestino se supportato dal provider.
- Copie GIO senza `OVERWRITE`, niente cancellazione automatica delle origini, symlink rifiutati in copie recursive, guardie su numero di elementi e profondità, interruzione mediante `gio::Cancellable` durante copie grandi.
- Preferiti URI in `~/.config/melania/gvfs-bookmarks` (o `XDG_CONFIG_HOME`) con scrittura atomica, permessi 0600 e controllo dei protocolli. Nessuna modifica ai preferiti GNOME/Nautilus.
- URI con password nella userinfo non accettati. Le credenziali devono essere richieste da GVfs tramite i suoi servizi desktop.
- Test Rust per enumerazione reale GIO `file://`, rifiuto sovrascrittura, validazione URL e preferiti. Test end-to-end GTK/Wayland con tab GIO reale su directory effimera in CI.

## Requisiti sulla distribuzione
Dipendenze esistenti: Rust, GTK4/GIO e compositore Wayland annidato. Per SMB/SFTP/WebDAV occorre installare **GVfs e i backend specifici** della propria distribuzione (ad esempio `gvfs-backends` e `gvfs-fuse` dove previsti), con sessione utente D-Bus. Non vengono installati server o servizi di rete da Melania. Nessuna credenziale deve essere salvata nei preferiti.

## Limiti — non dichiarare parità completa con Nautilus
Il vecchio pannello locale rimane basato su `PathBuf` e non è ancora confluito completamente nel modello URI. Le schede di rete lavorano con GIO, ma non hanno ancora tutte le funzioni del browser locale (multi-selezione, schede preferiti comuni, anteprime, ricerca ricorsiva, vista dettagli avanzata). Non sono inclusi negoziazione MOVE cross-app, undo/redo, progressi per byte, condivisioni SMB con autenticazione interattiva interna, `mount_enclosing_volume` con prompt, riconnessione offline, espulsione smontaggio, ACL e metadata remoti avanzati. Le copie di directory parziali possono restare presenti quando cancellate o interrotte; questo viene comunicato, non cancellato automaticamente.

**Criterio per una parità reale:** tutte le viste e tutte le operazioni devono usare gli stessi `gio::File` e la stessa API GIO; test contro server isolati SMB/SFTP/WebDAV, dialoghi auth, D-Bus/portali e regression test per collisioni. Issue #78 resta aperta.
