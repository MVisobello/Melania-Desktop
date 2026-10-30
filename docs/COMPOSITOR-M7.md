# M7 — Melania 0.9: consolidamento di compositore, IPC e drag & drop

## Perché una milestone di stabilizzazione
Il progetto dispone di molte componenti ma non è stato compilato in un ambiente con Rust, GTK 4 e Smithay. Prima di ampliare il compositore introduciamo operazioni locali deterministiche e riduciamo i rischi di perdita dati dovuti a negoziazioni DnD ambigue.

### 1. Drag & drop
- `gtk::DragSource` pubblica `gdk::FileList` delle origini locali, ma offre soltanto COPY in questa release.
- `gtk::DropTarget` in Melania Files accetta **solo COPY**, con evidenziazione delle cartelle di destinazione.
- I comandi **Taglia/Incolla** in Melania Files sono separati dal protocollo DnD. Si basano su `move_local`, con `renameat2(..., RENAME_NOREPLACE)` e errore sui filesystem diversi: non cancellano accidentalmente l'origine.
- File remoti GVfs, `text/uri-list` tra processi, drop in taskbar, XWayland e MOVE interapp: ancora da verificare e sviluppare.

### 2. IPC compositor/taskbar
`melania-control` accetta solo richieste terminate da un singolo LF, lunghe al massimo `MAX_COMMAND` byte. Rifiuta comandi parziali e contenenti più righe: un messaggio incompleto non deve mai diventare un comando valido.

**Ancora da fare:** eventi push, controllo credenziali peer, limite sul volume delle risposte generate dal server, versione esplicita del protocollo.

### 3. Verifica su Linux
1. `./scripts/check-melania-v09.sh` — test unitari e `cargo check --workspace`.
2. `./scripts/test-file-transfers-v09.sh` — anteprima, copia, conflitto, MOVE e nessuna sovrascrittura su file temporanei.
3. `./scripts/run-melania-v09.sh` — sessione annidata, solo dopo compilazione riuscita.
4. Test manuale di GTK, Qt, popup, layer shell, drag e drop file → cartella, file → applicazione, taskbar e protocollo DnD reale.

### Stato
Packaging, manifest e script: verificabili staticamente. Cargo/GTK/Wayland runtime: **non disponibili** nell'ambiente di creazione. Nessuna dichiarazione di funzionamento finale.
