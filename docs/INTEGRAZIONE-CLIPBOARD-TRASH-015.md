# Melania Files 0.15 — Appunti, Cestino e drag & drop

**Stato:** nuove implementazioni nei sorgenti Rust/GTK; non ancora compilate o collaudate su un desktop GTK reale. Non presentare come compatibilità completa con Qt/XWayland.

## Nuovo comportamento

- **Copia selezione:** pubblica un `GdkFileList` negli appunti dell'utente tramite `GdkClipboard::set_content`; conserva anche l'operazione nel buffer interno della finestra.
- **Incolla qui:** legge in modo asincrono `GdkFileList` dal *clipboard corrente* (così non sovrascrive la selezione appena copiata da un'altra applicazione). Accetta solo `GFile` con percorso locale. Utilizza `melania-transfer::copy_sources` (pianificazione preventiva, rifiuto sovrascrittura).
- **Taglia selezione:** resta volutamente un'operazione *interna a Melania Files*; non negozia MOVE fra applicazioni. All'incolla usa `renameat2(RENAME_NOREPLACE)` per lo spostamento locale atomico. Trasferimenti fra filesystem distinti non sono ancora supportati.
- **Sposta nel Cestino:** usa `gio::File::trash`. Se il dispositivo o il mount non supporta il Cestino, mostra un errore e **non passa a cancellazione permanente**. La cartella `trash:///` viene aperta tramite `xdg-open`.
- **Drag & drop:** `GtkDropTarget` e `GtkDragSource` pubblicizzano ancora esclusivamente `COPY`. Il file manager non cancella gli originali dopo un drag.
- **Icone:** tipologie di file e cartelle usano le icone del tema Linux, non emoji fisse.
- **Cartelle personali:** si risolvono tramite `xdg-user-dir` (Scrivania, Documenti, ecc.), con fallback esplicito.

## Limiti

1. `GdkFileList` supporta diversi formati MIME convertibili da GDK, ma interoperabilità completa fra tutte le app GTK, Qt, X11 e sandbox Flatpak **non è stata verificata**.
2. La copia e `gio::File::trash` usano ancora API sincrone, quindi operazioni su file grandi possono bloccare temporaneamente la finestra GTK. È necessario passare a job asincroni con progresso/annullamento prima della versione 1.0.
3. Gli appunti CUT interni non seguono ancora il protocollo `x-special/gnome-copied-files`; solo COPY viene pubblicato nel clipboard di sistema.
4. Il file manager gestisce attualmente una selezione singola nei comandi, mentre il drop può ricevere più file. Multi-selezione e batch actions restano nella roadmap.
5. La sincronizzazione dello stato CUT con i cambiamenti del clipboard esterno richiede un test UI reale. Evitare di utilizzare CUT con dati importanti finché non è collaudato.

## Test richiesti su Linux (nessun test simulato)

1. `cargo check -p melania-files --all-targets`
2. Aprire due finestre Melania Files e copiare/incollare un file; controllare checksum e originale.
3. Da un altro file manager GTK copiare un file e incollarlo in Melania. Ripetere con Dolphin/Qt.
4. Inviare al Cestino file e cartella, quindi recuperarli dal cestino di sistema; testare un filesystem senza supporto trash.
5. Drop di tre file, uno in conflitto: nessun file preesistente deve essere sovrascritto; nessun sorgente cancellato.
6. Verificare cartelle personali in una sessione Linux italiana, sia con `xdg-user-dir` disponibile sia senza.
7. Su XWayland e Flatpak eseguire almeno un trasferimento `text/uri-list` in entrambe le direzioni, annotando i formati negoziati.

## Riferimenti tecnici

- https://gtk-rs.org/gtk4-rs/stable/0.11/docs/gdk4/struct.Clipboard.html
- https://gtk-rs.org/gtk-rs-core/stable/latest/docs/gio/prelude/trait.FileExt.html
- https://specifications.freedesktop.org/desktop-entry-spec/latest/
