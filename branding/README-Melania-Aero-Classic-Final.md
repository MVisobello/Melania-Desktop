# Melania Aero Classic — Icon Pack definitivo

Tema icone Linux costruito usando **la tavola approvata dall’utente**, con un aspetto Aero classico più sobrio.

## Cosa contiene

- **47 disegni separati** dalla tavola finale approvata: 9 desktop, 18 sidebar, 20 applicazioni generiche.
- Icone PNG con trasparenza nei formati 16, 22, 24, 32, 48, 64, 96, 128, 256 px.
- Icone applicative riconoscibili della precedente v4 (Chrome, Chromium, Firefox, VLC, Thunderbird, VS Code, Spotify, Steam, Telegram e altre) preservate.
- Nomi freedesktop e fallback a Adwaita/hicolor.
- Icone piccole realmente derivate dalla **seconda riga di artwork** dedicata alla sidebar, non da icone grandi rimpicciolite.

## Installazione

```bash
./install.sh
```

Il tema viene copiato in `~/.local/share/icons/Melania-Aero-Classic`. Non viene attivato automaticamente. Dopo l'installazione, selezionalo nelle impostazioni del desktop.

## Note tecniche

- La grafica approvata è un'immagine raster: i file individuali sono **PNG**, non SVG vettoriali nativi.
- Le versioni a 256 px sono raster ridimensionizzati con Lanczos, non nuove illustrazioni a risoluzione 256 nativa.
- Le vecchie varianti `-symbolic` SVG sono mantenute per GTK, ma non vengono confuse con le nuove icone piccole a colori.
- Le applicazioni prive di un'icona dedicata conservano il proprio logo originale tramite fallback. Alcuni alias della precedente versione che mostravano loghi di programmi diversi sono stati rimossi per evitare riconoscimenti errati.
- Questo è un tema grafico indipendente: l'installazione non modifica driver, compositor o file manager.
- I loghi di terze parti non vengono rivendicati come marchi Melania.

## Cartelle

- `Melania-Aero-Classic/`: tema installabile.
- `sources/approved-icons/`: 47 PNG sorgente isolati.
- `sources/approved-reference.png`: tavola grafica approvata.
- `manifest.json`: disegni, alias e controlli.
- `anteprima-definitiva.png`: catalogo.
