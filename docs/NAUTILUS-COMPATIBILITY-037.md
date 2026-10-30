# Melania Files v0.37 — compatibilità funzionale con GNOME Files (Nautilus)

## Obiettivo e vincoli

Vogliamo **parità funzionale graduale**, senza copiare la UI di Nautilus: Melania conserva le tab browser-like, Aero e l'icon pack. Nautilus 51 è un file manager maturo con decenni di funzionalità e integrazione di GNOME; **non è corretto dichiarare la parità totale dopo questa iterazione**.

Questa tabella distingue sorgenti e comportamento verificato. I nuovi sorgenti GTK **non sono compilati** nell'ambiente di sviluppo; i test Python verificano la struttura e la stabilità del vecchio prototipo X11, **non** le nuove operazioni GTK.

| Funzione | Stato nei sorgenti v0.37 | Da completare / verificare |
|---|---|---|
| Navigazione locale, cronologia indipendente, schede | Presente | Build GTK e Wayland |
| Griglia ed elenco, filtri, ordinamento | Presente | Lista tabellare ricca, vista ad albero |
| Clic singolo selezione, doppio clic apertura | Presente | Interazione GTK reale |
| Copia/taglia/incolla e trascinamento | Presente, locale, COPY-only in DnD | Multi-selezione, progressi, annullamento, MOVE cross-app sicuro |
| Cestino recuperabile | GIO trash nei sorgenti | Ripristino interno, svuotamento e supporto server |
| Menu contestuale file | Aggiunto (Apri, Copia, Taglia, Rinomina, Cestino, Proprietà) | Compilazione, menu di sfondo, Open With esteso |
| Proprietà file | Aggiunte (tipo, percorso, dimensione, permessi POSIX in sola lettura, link) | Editor di permessi, ACL, associazioni MIME, proprietà cartella ricorsive |
| Rilevamento cambi cartella | Aggiunto monitor GIO con debouncing | Test ciclo di vita e carichi elevati |
| Volumi/dispositivi montati | Aggiunta vista dei mount locali tramite GIO | Montaggio/smontaggio, espulsione sicura, autenticazione |
| Ricerca per nome | Locale in cartella, ricerca ricorsiva limitata aggiunta | Indicizzazione, contenuto documenti, cancellazione asincrona, ricerca remota |
| Preferiti | Persistenza Melania | Migrazione/integrazione bookmark GTK e URI remoti |
| Creazione cartelle e file | Presente, usa create_new per i file | Template picker evoluto |
| Nuovo file da Modelli | Aggiunta copia esclusiva per file regolari | Monitor Modelli e interfaccia di scelta nome |
| Rinomina multipla | Pianificazione preflight non distruttiva nella libreria Rust | UI preview, selezione multipla, applicazione e rollback |
| Disco SMB/SFTP/WebDAV/NFS | **Non ancora presente nel browser** | Migrazione da PathBuf a `gio::File` e GVfs |
| Tree View, supporto archivi | **Mancante** | Modello albero, File Roller/libarchive |
| Script, plugin, condivisione | **Mancante** | Extension API e permessi/sandbox |
| Miniature video/PDF/audio | Solo immagini e testo parziale | Thumbnailer di sistema, caching, policy rete |
| Accessibilità/HiDPI/Wayland | Non collaudati | Test reale Rust/GTK/Smithay |

## Sicurezza delle operazioni

- Mai sovrascrivere silenziosamente i file: copia con `create_new`, rinomina/move locali senza replace su Linux.
- Non attivare `MOVE` nel protocollo DnD quando viene eseguita una copia.
- Non seguire directory collegate tramite symlink durante ricerche ricorsive.
- No rimozione definitiva automatica quando il Cestino non è disponibile.
- Ricerca limitata per risultati (150) e cartelle (120) nella UI; ancora sincrona, quindi evitare cartelle immense fino al passaggio a worker cancellabile.
- La pianificazione della rinomina multipla **non rinomina nulla**. L'operazione non viene esposta finché non saranno sviluppati preview, rollback e test.

## Fasi successive indispensabili

1. **Compilazione reale** Rust/GTK e correzione degli eventuali errori di API.
2. **Selezione multipla + job asincroni** con cancel/progress, drag & drop tra applicazioni.
3. **Backend GIO URI** per SMB/SFTP/WebDAV, gestione mount/unmount.
4. Viste ad albero, archivio, proprietà estese, indicizzazione e integrazione con thumbnailer.
5. Test Fedora/Mint/Wayland/XWayland, test di grandi directory e recupero dopo errore.

Riferimenti funzionali: https://apps.gnome.org/Nautilus/ e https://help.gnome.org/gnome-help/files.html
