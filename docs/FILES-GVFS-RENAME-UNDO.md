# Melania Files — Annulla/Ripeti rinomina (1 novembre 2026)

La scheda GIO/GVfs di Melania Files dispone ora di **Annulla rinomina** e **Ripeti rinomina** con le scorciatoie Ctrl+Z e Ctrl+Maiusc+Z, senza sottrarre queste combinazioni ai campi di testo GTK.

## Garanzie e comportamento

- È conservata **una sola rinomina reversibile per scheda**, solo in memoria, senza cronologia persistente e senza modificare altre applicazioni.
- Una rinomina normale continua a funzionare anche quando un provider non fornisce gli attributi `id::file` e `id::filesystem`; in quel caso il file viene rinominato ma **non è possibile annullare** in modo verificabile e il comando non viene abilitato.
- Una registrazione per Annulla viene creata soltanto quando questi identificativi rimangono uguali prima e dopo la rinomina.
- Prima di Annulla o Ripeti il backend controlla di nuovo l'identità dell'oggetto sorgente e rifiuta l'operazione se l'originale manca, è stato sostituito, o il nome di destinazione è occupato. La rinomina locale su Linux usa `renameat2(RENAME_NOREPLACE)`.
- Le operazioni non possono cancellare o sovrascrivere deliberatamente file preesistenti; è comunque necessario un test reale per ciascun provider remoto, perché le garanzie atomiche e i segnali di identità del server variano.
- Dopo altre modifiche completate tramite Melania Files, la cronologia della rinomina viene azzerata. Non è ancora un journal completo per copie, cancellazioni, cestino e spostamenti.

## Test e limiti

I test Rust verificano rinomina, annullamento e ripetizione con file reali, protezione collisioni e sostituzione successiva dell'oggetto originale; i test Python controllano il wiring della GUI. Build GTK e sessioni Wayland sono richieste prima del merge.

Il supporto è **condizionato agli attributi del provider**. Non è garantito che SFTP/SMB/WebDAV li forniscano. Un controllo identità + rinomina remota può comunque avere una finestra di gara (TOCTOU) se il server non implementa l'operazione in modo atomico. Non dichiarare ancora parità Nautilus.
