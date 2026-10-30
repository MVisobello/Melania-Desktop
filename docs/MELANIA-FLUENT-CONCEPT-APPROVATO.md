# Melania Fluent — tema ufficiale dal concept approvato

L'autore ha approvato **l'immagine del 30 ottobre 2026** (mappamondo azzurro per il browser, Download nero, Musica verde, Video rosso, Immagini celeste, Desktop azzurro). Questo è il riferimento visivo prioritario, NON la precedente serie di bozze generative Aero o Fluent.

## Implementazione
- `tools/melania_fluent_reference.py`: 36 immagini effettivamente visibili nell'immagine approvata, estratte e convertite in PNG con alpha (160 colori, 192×192 pixel), incorporate nelle icone SVG.
- `tools/build_melania_fluent_themes.py`: usa **prima** gli asset approvati e solo dopo la famiglia di disegni generati dal modulo `melania_fluent_artwork`.
- `tools/test_melania_fluent_reference.py`: testa dimensioni, PNG integrati, SVG e alias essenziali.
- Inventari `branding/coverage/reference-{dark,light}-paths.txt`: solo nomi dei file, non disegni Flat Remix.
- Varianti `Melania-Fluent-Light`, `Melania-Fluent-Dark` e `Melania-Fluent-Light-on-Dark-Panel`. Le icone simboliche e di sistema restano vettoriali e con contrasto specifico per tema.
- `melania-config` usa ora **Melania-Fluent-Light** come predefinito. `melania-settings` espone tutti e 6 i temi, compresi quelli Aero precedenti.

## Limite artistico importante
**Questo primo rilascio è fedele ai pixel del mockup, non è un SVG vettoriale nativo.** I 36 soggetti ricavati dal mockup sono immagini a risoluzione limitata dentro contenitori SVG. Le icone dell'inventario non visibili nell'immagine vengono risolte dal generatore precedente. Non presentare questa prima versione come 21.011 illustrazioni indipendenti né come un set vettoriale completo.

Per una seconda fase, i 36 soggetti possono essere tracciati singolarmente come SVG vettoriali usando il mockup approvato come modello, SENZA cambiare geometrie, simboli e colori.

## Generare e installare
```bash
python3 tools/test_melania_fluent_reference.py
python3 tools/test_melania_fluent_artwork.py
python3 tools/build_melania_fluent_themes.py --output dist/melania-fluent-icons
bash scripts/install-melania-fluent-icons.sh dist/melania-fluent-icons
```
Il tema predefinito usa l'installazione utente in `~/.local/share/icons`. L'installazione non cambia il tema GTK del desktop host, soltanto i temi disponibili a Melania.

La pipeline `Build Melania Fluent icon themes` produce il pacchetto freedesktop in formato `.tar.xz`.
