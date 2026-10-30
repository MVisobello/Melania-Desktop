# Melania Aero Icons — specifica e installazione (v0.20)

## Obiettivo
Set di icone proprietario per Melania Desktop: look Aero moderno, riconoscibile, non copiato da Windows, coerente con la mezzaluna blu scelta come marchio Melania.

## Contenuto
- **96 icone colorate** (92 disegni per categorie più 4 alias/app brand), tutte con SVG modificabile.
- **13 varianti `-symbolic`** in SVG monocromatico, usabili su pannelli GTK e in stati attivo/disattivo.
- **672 file PNG** (96 icone per sette dimensioni: 16, 24, 32, 48, 64, 128 e 256 px).
- Un file `index.theme` conforme al formato freedesktop, con fallback `Adwaita,hicolor`.
- Icone a 16 e 24 px disegnate in forma semplificata: non si riduce soltanto la grafica grande.
- Anteprime `branding/melania-icons-banner.png` e `branding/anteprima-melania-icons.png`.

## Come installare (senza modificare Cinnamon)
Dalla cartella principale del progetto:

```bash
./scripts/install-melania-icons.sh
./scripts/install-melania-icons.sh --check
```

Il tema viene copiato in `${XDG_DATA_HOME:-~/.local/share}/icons/Melania`. L'impostazione del tema in Cinnamon **non viene cambiata** automaticamente.

Se vuoi scegliere Melania come tema icone per *tutto* il desktop GTK corrente, dopo aver valutato l'effetto sulle altre applicazioni:

```bash
./scripts/install-melania-icons.sh --activate
```

Il secondo comando modifica la preferenza GTK `org.gnome.desktop.interface icon-theme` quando disponibile. Per tornare al tema precedente, selezionalo dalle impostazioni del desktop. L'installazione delle sole icone è indipendente dal compositor Smithay.

## Sviluppo / generazione

```bash
python3 tools/build_icon_theme.py
python3 tools/preview_icon_theme.py
python3 tools/preview_icon_banner.py
python3 tools/test_icon_theme.py
```

Per rigenerare PNG e anteprime serve `cairosvg` e `Pillow` in ambiente di sviluppo. Le applicazioni **non richiedono Python** per utilizzare le icone già installate. Per rigenerare solo SVG:

```bash
python3 tools/build_icon_theme.py --svg-only
```

## Integrazione GTK
Nel codice `melania-files`, `melania-desktop-surface`, `melania-panel` e `melania-settings` richiamano `setup_melania_icon_theme()` nella funzione che costruisce la finestra. Questa aggiunge la directory sorgente del tema alla ricerca GTK durante lo sviluppo; il tema può inoltre essere trovato nella normale directory XDG dopo l'installazione.

Le icone vengono referenziate per nome freedesktop (ad es. `folder-documents`, `application-pdf`, `network-wireless`). Le icone di programmi esterni non disegnati da Melania vengono cercate nei temi ereditati: non vengono sostituite arbitrariamente.

## Regole per i prossimi disegni
- Mai usare materiale protetto di Microsoft, Apple o altri produttori.
- Le silhouette devono essere riconoscibili a 16 px.
- Privilegiare blu, azzurro, ardesia e variazioni cromatiche funzionali (verde fogli di calcolo, rosa PDF, viola fotografie).
- Accessibilità: le icone non devono essere l'unico indicatore di stato; prevedere testo, tooltip e focus tastiera.
- Evitare effetti costosi a runtime: le icone sono asset statici, non animazioni.

## Licenze e stato
I nuovi disegni generati per Melania sono contenuto originale del progetto. Il **logo Melania** deriva dal concept fornito dall'utente e resta un asset distinto: l'inclusione nel pacchetto non implica una cessione di diritti. Le altre icone ereditate appartengono ai rispettivi progetti; non sono copiate nel nostro set.

**Qualità verificata:** validazione XML, dimensioni PNG, integrità dei file, corrispondenza dei nomi GTK, test di installazione in cartella temporanea. **Non ancora verificato:** rendering in una build compilata Rust/GTK, sessione Wayland e qualità su diversi display HiDPI reali.
