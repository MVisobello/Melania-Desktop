# Melania Aero — UX Lab verificato

Questo è il **prototipo visivo interattivo** di Melania, costruito per verificare il linguaggio grafico del concept approvato: menu Start a due colonne, taskbar Aero, file manager chiaro con sidebar, anteprime delle finestre, centro rapido e calendario.

## Cosa è vero e cosa no

- L'applicazione viene **realmente avviata** tramite Python/Tk su X11, anche in una sessione Xvfb.
- Il progetto usa **file PNG del Melania Aero Classic Icon Pack FINAL**, presenti in `assets/Melania-Aero-Classic/`.
- I pulsanti Start, Files, Impostazioni, pannello rapido, calendario, selezione delle miniature, toggle e desktop virtuali dimostrativi rispondono ai clic.
- La sfocatura delle superfici è calcolata con Pillow sui pixel dello sfondo *del prototipo*, **non dal renderer Wayland/Smithay**.
- Le cartelle e i documenti nell'interfaccia Files sono contenuto dimostrativo ispirato al concept inviato dall'utente; le miniature provengono dai riferimenti visivi forniti nella conversazione.
- **Non** è una compilazione di Melania Rust/GTK, **non** sostituisce Cinnamon e **non** dimostra la compatibilità reale Wayland, Qt, XWayland o il drag & drop cross-app.

## Avvio su Linux

Richiede Python 3, Tk e Pillow.

```bash
sudo apt install python3-tk python3-pil
python3 melania_aero_live.py
```

Se usi un venv, installa Pillow al suo interno. La schermata è disegnata per 1536 × 864 px.

## Controlli implementati nel prototipo

- Clic sul pulsante Melania → apre/chiude il menu Start.
- Clic sulle icone della taskbar → apre/chiude Files o Impostazioni; attiva/disattiva le anteprime.
- Clic sulle icone del file manager → cambia la selezione e il pannello dettagli.
- Clic sulla sidebar → aggiorna la sezione selezionata.
- Clic sui toggle → cambia lo stato dimostrativo Wi‑Fi, Bluetooth ecc.
- Clic sul calendario/orologio → apre/chiude le rispettive sezioni.
- Clic sui numeri 1–4 della taskbar → cambia spazio di lavoro nel modello UI.
- Esc → chiude menu e pannelli.

Le azioni sono registrate in `events.jsonl`, senza modificare le impostazioni di rete o i file del sistema.

## Test

```bash
# Su X11 o su un display virtuale Xvfb:
python3 test_aero_live.py
```

Il test verifica il caricamento delle icone definitive e le principali azioni della UI. `screenshot-verificato.png` è acquisito dalla finestra in esecuzione su un display Xvfb, **non generato con un motore di concept art**.

## Provenienza risorse

- Melania Aero Classic FINAL: icone approvate dall'utente, incluse nella directory `assets/Melania-Aero-Classic`.
- Logo Melania: da `branding/icons/melania-128.png` del repository.
- `wallpaper.png`: sfondo originale del progetto Melania (non ancora lo sfondo fotografico del concept).
- `reference.png`: immagine di riferimento fornita dall'utente, utilizzata per ritagliare le miniature del file manager. Queste fotografie restano materiale dimostrativo e non vengono distribuite come contenuto autonomo.

## Prossima integrazione tecnica

1. Sostituire il renderer Tk con componenti GTK4/Layer Shell e realizzare la cattura dello sfondo con il compositore Smithay.
2. Collegare i pulsanti della taskbar all'elenco di finestre reali e agli eventi Wayland.
3. Integrare il file manager Rust con miniature e d&d sicuro, già impostato nelle versioni precedenti, verificandolo su sessione reale.

## Aggiornamento v0.29
Ricerca reale nel menu Start e nel prototipo Files, alternanza griglia/elenco, pannello dettagli controllabile, anteprima di Files ricavata dalla finestra disegnata e migliore comportamento a 800×600. Vedere `test_029_interactions.py` e `capture_029.py`.
