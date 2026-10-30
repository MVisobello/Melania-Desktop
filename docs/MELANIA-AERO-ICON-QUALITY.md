# Melania Desktop — Aero icons: rifinitura grafica e QA

## Obiettivo

Tre temi installabili: **Melania Light**, **Melania Dark** e **Melania Light on Dark Panel**.
Identità di Melania Desktop ispirata al realismo leggero e alle superfici Aero di
Windows 7, con disegni originali. Flat Remix fornisce unicamente l'inventario
di nomi e cartelle per la compatibilità; nessuna grafica viene importata.

## Cosa è stato aggiornato

- Cartelle dorate con linguetta, spessore, bordo e riflessi; emblemi specifici
  per Home, Documenti, Download, Immagini, Musica, Video e condivisione.
- File di testo, PDF, documenti, fogli di calcolo, presentazioni, archivi, file
  multimediali e codice, con famiglie cromatiche distinte.
- Computer, notebook, monitor, stampante, tastiera, mouse, dischi e USB.
- Cestino vuoto/pieno, server, gruppo di lavoro, ricerca salvata e dischi ottici.
- Comandi di Files/Explorer con forme autonome senza tasselli quadrati.
- Indicatori Wi-Fi, audio, batteria, Bluetooth e rete con alto contrasto Light/Dark.
- Icone categoria distinte (internet, office, sviluppo, grafica, sistema,
  giochi, istruzione ecc.), invece di un unico pittogramma generico.
- Piccole versioni disegnate appositamente a 16 e 24 px per le icone principali,
  con geometrie e spessori semplificati. Le SVG scalabili rimangono disponibili
  per 48, 64, 96, 128 e altre dimensioni.

## Correzione importante: collegamenti tra varianti

Le icone colorate condivise fra Dark e Light ora sono collegate **file per
file**. Non si tenta più di collegare l'intera sottocartella se al suo interno
esistono icone simboliche specifiche per Light. In questo modo non scompaiono
gli altri nomi della stessa directory.

Status, panel e actions hanno varianti dedicate per il contrasto. Il tema
Light on Dark Panel eredita le icone delle finestre da Light e gli indicatori
del pannello da Dark.

## Produrre e verificare

```bash
python3 tools/test_melania_aero_artwork.py
python3 tools/test_melania_aero_details.py
python3 tools/build_melania_icon_variants.py
python3 tools/build_melania_icon_variants.py --verify
python3 tools/audit_melania_icon_variants.py --report dist/melania-icon-audit.json
```

Il comando `--verify` ora è non distruttivo: **non ricostruisce e non cancella
il pacchetto esistente**.

Per una tavola di controllo che rasterizza realmente i SVG generati:

```bash
python3 -m venv .venv-icons
.venv-icons/bin/pip install pillow cairosvg
.venv-icons/bin/python tools/preview_melania_variants.py
```

La workflow **Build Melania Aero icon themes** esegue test, generazione,
verifica, audit, anteprima e pacchettizzazione. In caso di esito positivo
pubblica l'artifact **Melania-Aero-Icon-Themes**, con:

- `Melania-Aero-Complete.tar.xz`: tutti e tre i temi, con link relativi
- `melania-icon-audit.json`: copertura, integrità e differenze
- `Melania-Aero-Visual-QA.png`: anteprima delle icone effettive a 16/24/48/96 px

## Limiti e prossimo controllo

La copertura dei nomi è distinta dalla qualità grafica: le icone sviluppate
individualmente convivono con altre icone originali Melania e con simboli
generati automaticamente. Gli identificatori alias possono condividere
lo stesso disegno. Non si dichiara che tutti i 20.000+ nomi siano illustrazioni
indipendenti.

La verifica del codice da sola non equivale a una prova grafica sul desktop:
servono il report della pipeline, un'ispezione della tavola PNG e il controllo
a schermo in Melania Files, Settings e Panel, sia Light sia Dark.
