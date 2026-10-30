# Melania — pacchetti icone Light, Dark, Light on Dark Panel

## Identità originale
Il riferimento Flat Remix Yellow (archivio del 19 novembre 2025) serve **solo a
catalogare nomi, contesti e collegamenti**. Non sono stati copiati SVG, PNG,
disegni, palette o gradienti da Flat Remix.

La grafica di Melania usa le risorse già presenti in
`branding/Melania-Aero-Classic` e `branding/Melania` (cartelle Windows 7/Aero,
riflessi, dispositivi, categorie, logo della mezzaluna) integrate da simboli
vettoriali originali generati in blu/vetro. Le icone terze già incorporate nel
precedente pack Melania rimangono provenienti da *quel pack*; rispettare le
relative licenze anche nella distribuzione pubblica.

## Copertura verificata
Il builder usa un inventario testuale dei nomi derivato dal riferimento:

- **20.364 percorsi SVG** nella variante di riferimento Dark.
- **9.973 percorsi SVG** nella variante di riferimento Light.
- Le differenze sono colmate in entrambe le varianti: i nuovi temi espongono
  **l'unione** dei nomi, invece di riprodurre assenze delle varianti originali.
- L'inventario comprende Actions (16,18,22,24, symbolic), Apps, Animations,
  Categories, Devices, Emblems, MimeTypes, Panel, Places e Status.
- Gli alias duplicati usano link simbolici ai file SVG Melania: la compatibilità
  dei nomi **non implica** che ogni alias abbia un disegno unico fatto a mano.
  Le icone mancanti vengono risolte con motivi vettoriali di Melania.
- L'inventario e i test sono indipendenti dai file grafici Flat Remix.

## Varianti di tema freedesktop

| Directory | Nome mostrato | Utilizzo |
|---|---|---|
| `Melania-Light` | Melania Light | Finestra chiara, icone comandi scure |
| `Melania-Dark` | Melania Dark | Finestra scura, icone comandi chiare |
| `Melania-Light-on-Dark-Panel` | Melania Light on Dark Panel | Comandi Light, status e pannello Dark |

Le icone colorate delle applicazioni e le cartelle conservano lo stesso stile
classico e possono essere condivise fra varianti. I simboli di azione, stato,
dispositivi e pannello hanno invece versioni ad alto contrasto specifiche:
scure sulle finestre Light, chiare sulle superfici Dark. La terza variante
combina simboli Light nelle finestre con status/pannello Dark, attraverso
collegamenti alle due precedenti. **Installare sempre le tre directory assieme.**

## Costruzione e installazione

Dalla root del repository:

```bash
python3 tools/build_melania_icon_variants.py
bash scripts/install-melania-icon-variants.sh
```

L'installazione avviene solo in
`$XDG_DATA_HOME/icons` (predefinito `~/.local/share/icons`).
Non modifica GNOME, Cinnamon, XFCE, KDE o le preferenze del desktop host.

In Melania: **Impostazioni → Aspetto → Pacchetto icone Melania** e selezionare
la variante. La finestra Settings applica subito la selezione se le icone
sono installate; Files, Panel e Desktop Surface la leggono al riavvio.
Se il tema manca, si usa il precedente Melania Aero Classic come fallback.
Le vecchie preferenze rimangono leggibili.

## Scaricare il pacchetto pronto
La workflow `Build Melania Aero icon themes` produce l'artifact
`Melania-Aero-Icon-Themes` che contiene `Melania-Aero-Complete.tar.xz`.
Le directory interne preservano i collegamenti relativi; per installare
estrarre in una cartella e passare il percorso allo script d'installazione.

## Criteri di qualità
- Copertura di tutti i percorsi degli inventari in ogni variante.
- Esame XML SVG (nessun collegamento a icone Flat Remix).
- Differenza di leggibilità delle icone d'azione light/dark.
- Logo e cartelle originarie Melania mantenuti.
- Installazione isolata senza modificare il tema del desktop host.
- Compilazione GTK4 di Files, Settings, Panel e Desktop Surface.
- Esecuzione e visual inspection in nested Wayland.

## Limiti trasparenti
Molti dei nomi compatibili sono alias o icone generate automaticamente e
richiederanno rifiniture artistiche manuali nelle successive versioni. Per
essere un pack professionale serve un ciclo di QA visiva per ogni gruppo
(App, Devices, Actions, Status). Questo è un **set completo per copertura
tecnica**, non 20.000 illustrazioni indipendenti disegnate a mano.
