# Melania Desktop 0.38 — Files: selezione multipla e copie in background

Questa distribuzione include il sorgente completo del progetto Melania precedente, con una revisione di **Melania Files**. Il prototipo UX Lab e gli asset Aero Classic sono mantenuti, senza cambiamenti grafici intenzionali.

## Modifiche v0.38

- Selezione multipla per scheda con Ctrl+clic, Shift+clic e Ctrl+A, sincronizzata ai filtri di ricerca e alla vista attiva.
- Trascinamento di più file selezionati, sempre e solo COPY fra applicazioni.
- Copia di più elementi negli appunti di sistema tramite `gdk::FileList`.
- Copia locale con worker Rust separato dal thread GTK, eventi di progresso e pulsante **Annulla copia**.
- Preflight per i nomi già presenti, scritture `create_new` senza sostituire file, conservazione dei link simbolici senza seguirli e tentativo di pulizia dei soli nuovi output su errore o annullamento.
- Spostamento locale di più elementi mediante `renameat2(RENAME_NOREPLACE)` **uno per volta**. Se un elemento fallisce, i precedenti potrebbero essere già stati mossi: l'operazione non è una transazione globale e viene comunicata come parziale.
- Spostamento di più elementi nel Cestino GIO senza ripiegare sulla cancellazione definitiva.
- Pulsante di avanzamento e annullamento per le copie locali e aggiornamento del file manager al termine del lavoro.
- Menu contestuale compatibile con la selezione multipla già attiva.

## Limiti non risolti

1. **Rust/GTK non compilato in questo ambiente**: Cargo e gli header GTK 4 non sono disponibili. Le nuove funzioni sono presenti nel codice ma richiedono una build e test funzionali prima dell'uso su dati importanti.
2. Gli **spostamenti** restano locali, sullo stesso filesystem, atomici per singolo file. Un MOVE fra filesystem e lo spostamento DnD fra applicazioni non sono abilitati.
3. Il cancellamento della copia prova a eliminare gli output creati; **la pulizia è best effort** in caso di errori di filesystem, permessi o interventi di processi concorrenti. Verificare eventuali residui.
4. SMB/SFTP/WebDAV richiedono ancora il passaggio del backend a `gio::File`/GVfs; il progetto non offre ancora parità completa con Nautilus.
5. Il prototipo X11 continua a mostrare l'interfaccia dimostrativa e non rappresenta l'esecuzione dei nuovi componenti Rust.

## Test

Dal progetto:

```sh
python3 -m unittest discover -s tests -p 'test_*.py'
```

Per eseguire i nuovi unit test Rust, in un Linux con toolchain:

```sh
cargo test -p melania-transfer
cargo check -p melania-files
```

Test grafici X11 del laboratorio (richiedono Tk, Pillow e Xvfb):

```sh
cd ../UX-Lab
xvfb-run -a python3 -m unittest test_035_refinements.py test_032_tabs.py
```

## Sicurezza del backup

Lo ZIP non è un installer né un sistema già avviabile. Estrarre in una cartella di lavoro, conservando la v0.37 come ulteriore backup separato.
