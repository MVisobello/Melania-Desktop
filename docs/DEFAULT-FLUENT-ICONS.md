# Melania Fluent v0.4 — icone predefinite

Il pacchetto di icone originale **Melania Fluent v0.4** (SVG) è il tema predefinito di Melania nelle nuove configurazioni. Sono disponibili le tre varianti:

- **Light** → `Melania-Light`
- **Dark** → `Melania-Dark`
- **Aero** → `Melania-Light-darkPanel` (eredita gli SVG da Melania-Light)

Il pacchetto viene installato **solo nell'ambito di Melania**: non modifica i temi di Cinnamon, GNOME, XFCE o di un'altra sessione del sistema.

## Preparazione dell'archivio originale

Posizionare `Melania-Fluent-v0.4-Complete.tar.xz` in una cartella qualsiasi e lanciare:

```bash
python3 scripts/stage-melania-fluent-v04.py /percorso/Melania-Fluent-v0.4-Complete.tar.xz
```

Lo script convalida l'archivio, la topologia dei collegamenti SVG e la presenza dei tre temi. Crea `dist/melania-fluent-v04/`, aggiungendo alias per le vecchie versioni di Melania. `dist/` è uno spazio locale generato, non una modifica al tema GTK dell'host.

**Importante:** l'archivio completo è fornito separatamente: non è ancora contenuto nel repository. Una build eseguita senza questa fase mantiene il fallback compatibile `Melania-Preview` / `Adwaita`.

## Anteprima installabile

Dopo la compilazione dei binari, eseguire la procedura esistente:

```bash
python3 scripts/package-nested-preview.py --bin-dir target/debug --output-dir target/melania-preview-release
```

Il bundle risultante contiene gli SVG reali e i relativi collegamenti simbolici di tutti e tre i temi se sono stati precedentemente preparati con lo script. I processi GTK di Melania utilizzano `MELANIA_BUNDLE_DATA_DIR/share/icons`; le preferenze del desktop ospitante non vengono alterate.

**Comportamento atteso:**
- primo avvio (Aero): icone Fluent Light su pannello scuro;
- cambio Light: Fluent Light;
- cambio Dark: Fluent Dark;
- ritorno Aero: Fluent Light-darkPanel;
- configurazioni storiche: alias verso Fluent v0.4 oppure scelta esplicita del tema precedente;
- se gli SVG completi non sono stati preparati: la build continua a usare i fallback già presenti e dichiara nel manifest `bundled_fluent_v04=false`.

Per verificare quale pacchetto è stato effettivamente incluso, leggere `manifest.json` nel bundle: `bundled_icon_theme` e `bundled_fluent_v04`.
