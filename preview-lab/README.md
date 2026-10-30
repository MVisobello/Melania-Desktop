# Melania Preview Lab (testato)

Questo è un **prototipo interattivo temporaneo X11/Tkinter** del design Melania, NON il Desktop Environment Rust/GTK4. Serve a verificare visivamente UX, posizionamento delle finestre, pannelli, menu, ricerca delle applicazioni fittizie, Aero Snap illustrativo e drag & drop sicuro fra cartelle demo.

## Avvio

Prerequisiti: Linux con X11 o XWayland, Python 3, Tkinter, Pillow.

```bash
python3 preview-lab/melania_preview.py
```

Shortcut: `Ctrl+E` apre Files; `Ctrl+Alt+T` apre il Terminale dimostrativo. Trascina `Leggimi.txt` (nella cartella Home) sopra la cartella `Progetti`: viene creata una copia senza sovrascrivere. Muovi le finestre trascinandone il titolo: verso un bordo si attiva lo snap simulato. Il centro rapido si apre dall'area notifiche, a destra della taskbar; i suoi toggle sono **solo dimostrativi** e non modificano servizi host.

## Prove automatiche

```bash
python3 preview-lab/test_preview.py
./scripts/test-preview-lab.sh /tmp/melania-screenshot.png
```

Il secondo script richiede anche Xvfb e pyautogui, apre una finestra su un display virtuale e simula un vero drag & drop del mouse. La verifica si interrompe con errore se il file non viene copiato correttamente. Dopo la prova il filesystem temporaneo viene eliminato.

Screenshot verificato: `preview-lab/session-verified.png`.
