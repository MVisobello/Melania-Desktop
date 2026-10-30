# Melania Classic Aero — 30 icone principali (2026-10-30)

## Direzione grafica

Queste 30 famiglie di icone **sostituiscono le bozze grafiche generiche** per
gli elementi centrali del desktop. Le tre schermate di riferimento fornite
dall'autore di Melania Desktop raffigurano un ambiente Aero classico: Home
come casetta blu, Documenti come cartella giallo chiaro e Immagini come vera
fotografia con paesaggio, non una generica icona app.

Il codice originale vive in `tools/melania_reference_artwork.py`.
Le forme sono SVG originali vettoriali senza copie della grafica Flat Remix,
che continua a servire esclusivamente per la copertura dei nomi.

## 30 famiglie

Home, Documenti, Immagini, Musica, Video, Download, Recenti, Preferiti,
Desktop, Cestino, Cartella generica, Cartella aperta, Computer, Sistema,
Disco dati, Unità USB, Rete locale, Server, Dispositivi, Browser web,
File Explorer, Terminale, Posta, Impostazioni, Foto, Ufficio,
Indietro, Avanti, Aggiorna, Cerca.

I nomi freedesktop più comuni (es. `user-home`, `folder-documents`,
`folder-pictures`, `org.melaniadesktop.Files` e `go-previous`) sono
associati ai disegni principali. I nomi non coperti conservano il sistema
di icone già presente, **senza cancellare l'inventario esistente**.

## Varianti, dimensioni e stile

- **Light:** icone colorate tradizionali, simboli di navigazione blu.
- **Dark:** stessa famiglia di disegni colorati, contrasto aumentato
  sui comandi di navigazione.
- **Light on Dark Panel:** disegni di Light nelle finestre e simboli
  visibili sul pannello scuro.

Il generatore produce vettori compatibili con le dimensioni
16, 24, 32, 48, 64, 96, 128 e 256 pixel. La compatibilità di nomi
non significa che 20.000 nomi abbiano 20.000 disegni differenti.

### Comandi di verifica

```bash
python3 tools/test_melania_reference_artwork.py
python3 tools/build_melania_icon_variants.py
python3 tools/build_melania_icon_variants.py --verify
```

I temi freedesktop generati mantengono i nomi
`Melania-Light`, `Melania-Dark` e
`Melania-Light-on-Dark-Panel`.

L'archivio di anteprima standalone è separato dai temi di produzione:
usa i nomi `Melania-Classic-Light`, `Melania-Classic-Dark` e
`Melania-Classic-Light-on-Dark-Panel`, con ereditarietà sui temi
originali. Non è richiesto sovrascrivere gli icon pack di produzione
per ispezionare le prime 30 famiglie.

## Qualità e limiti

I nuovi SVG sono stati renderizzati per l'anteprima con CairoSVG e
convalidati come XML. I controlli unitari verificano le varianti,
l'unicità delle famiglie fondamentali e i nomi più usati.
La validazione automatica GitHub Actions e l'ispezione
nell'interfaccia GTK restano criteri distinti di accettazione.
