# Melania Desktop 0.19 — icone, selezione, menu contestuale, drag & drop

## Cosa cambia nei sorgenti

La superficie `melania-desktop-surface` è un client GTK4 di Layer Shell sul
livello `Background`, non una finestra di esempio X11. Questa iterazione aggiunge
la logica classica di un desktop utilizzabile senza modificare il progetto grafico
approvato.

### Interazioni delle icone

- **Un clic:** seleziona l'icona, senza lanciare l'applicazione.
- **Doppio clic:** apre il documento o la cartella con `xdg-open`.
- **Ctrl + clic:** selezione multipla, con evidenziazione CSS.
- **Drag di icona selezionata:** offre la selezione di massimo 256 file attraverso
  `gdk::FileList`, senza passare comandi testuali o shell.
- **Drop su cartella:** copia gli elementi nella cartella di destinazione.
- **Drop sullo sfondo:** copia nella cartella XDG Scrivania.

Le operazioni drag sono sempre **COPY**: l'origine non viene rimossa da alcun
handler GTK di questa release. MOVE, symlink remoti e operazioni cross-device
non sono attivati finché non vengono testati sul compositore reale.

### Menu contestuale

Un clic destro sulla superficie desktop presenta:

1. Nuova cartella (creazione atomica `std::fs::create_dir` con suffissi progressivi)
2. Aggiorna desktop
3. Apri Melania Files
4. Impostazioni

La creazione di cartelle non modifica o sostituisce file esistenti, neppure
in caso di operazioni concorrenti.

### Nuova libreria `melania-desktop-core`

Separata da GTK e Smithay. Contiene:

- listing senza ricorsione né attraversamento di link simbolici
- ordinamento stabile: cartelle, poi file per nome
- memoria limitata a N icone mostrate tramite inserimento ordinato
- nuova cartella con gestione degli eventuali conflitti
- modello di selezione basato su percorsi e non sugli indici della griglia

Il crate comprende **5 unit test Rust** (cartelle atomiche, concorrenza, sorting,
selezione e symlink). Essi **non sono stati eseguiti localmente**, poiché mancano
`cargo`, `rustc` e `libgtk-4-dev`.

### Scorciatoie del menu Start

Le directory locali Documenti, Immagini e Download sono ora individuate tramite
`xdg-user-dir`, con fallback sulle cartelle italiane e inglesi. In precedenza
si tentava impropriamente di leggerle da variabili d'ambiente non garantite.

## Test e limiti

```bash
./scripts/check-melania-v019.sh
```

Il comando esegue i test strutturali Python e, se disponibile Cargo, **esegue
i test Rust e `cargo check` GTK**. In assenza di Cargo termina con codice 3
(ben distinto da uno stato PASS). Il compositore Smithay non è verificato
in questa release e può ancora mostrare problemi su popup/input.

Test eseguiti nel container:
- 41 controlli strutturali Python: PASS
- 5 test del prototipo filesystem Python: PASS
- shell scripts: validazione Bash PASS
- Rust test/compile: NOT RUN (ambiente sprovvisto di Rust)
- GUI GTK4/Wayland: NOT RUN

## Prossimi blocchi prioritari

1. Compilare e correggere il workspace Rust con l'output effettivo di Cargo.
2. Eseguire la sessione annidata e verificare selezione, doppio clic e drop.
3. Supportare il selettore tastiera, azioni multiple e gestione popup completa.
4. Integrare COPY/MOVE fra applicazioni GTK, Qt e XWayland tramite Smithay.
5. In futuro: effetti Aero Glass reali lato compositore, gestione monitor, XDG portal.
