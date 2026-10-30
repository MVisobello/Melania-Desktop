# Melania Desktop 0.10 — Report della sessione verificata

**Data:** 29 ottobre 2026. **Piattaforma di verifica:** Debian GNU/Linux 13, Xvfb 1280×800×24, Python 3.13 e Tkinter/X11.

## Risultato effettivo

Un prototipo interattivo e isolato è stato **avviato e collaudato** in un server grafico X11 virtuale. Questo è un **UX test bench**, NON una sessione Wayland, NON l'applicazione GTK4 definitiva e NON il compositore Smithay.

| Prova eseguita | Esito |
|---|---|
| Avvio finestra e rendering desktop 1280×800 in Xvfb | PASS |
| Apertura finestre Melania Files e Impostazioni | PASS (visibili nello screenshot) |
| Taskbar + orologio + accesso al centro rapido | PASS (centro visibile nello screenshot) |
| Drag & drop con il **mouse** dal file `Home/Leggimi.txt` alla cartella `Home/Progetti` | PASS |
| Contenuto del file copiato corrisponde byte per byte all'originale | PASS |
| Originale ancora presente dopo la copia | PASS |
| Toggle Wi-Fi nel centro rapido | PASS come interazione DEMO, non tocca la rete del computer |
| Unit test del motore di copia: 5 test | PASS |
| Compilazione/avvio dei crate Rust/GTK4 | **NON ESEGUITI** |
| Avvio del compositore Wayland/Smithay | **NON ESEGUITO** |
| Drag & drop da applicazioni GTK4/Qt/XWayland reali | **NON ESEGUITO** |

## Limiti ambientali riscontrati

- `cargo` e `rustc` **assenti**; `pkg-config gtk4` **non disponibile**.
- Download con apt e curl non possibili per mancata risoluzione DNS.
- Nessun risultato di compilazione Rust può essere dedotto dai controlli statici.

## Come ripetere la prova grafica

Da un Linux con Xvfb, Python3, Tkinter, Pillow e pyautogui:

```bash
python3 preview-lab/test_preview.py
./scripts/test-preview-lab.sh screenshot.png
```

Per aprire manualmente il prototipo in una sessione Linux X11/XWayland:

```bash
python3 preview-lab/melania_preview.py
```

Tutte le prove del file manager utilizzano una directory temporanea cancellata all'uscita. La demo non modifica le impostazioni reali di rete, audio o display.

## Come iniziare la vera verifica Wayland

Su un PC Linux con gli strumenti di sviluppo:

```bash
./scripts/bootstrap-debian-dev.sh --install
cargo check --workspace --all-targets
cargo test --workspace
./scripts/run-melania-v09.sh
```

**Attenzione:** `run-melania-v09.sh` e il codice Smithay sono sperimentali e potrebbero richiedere correzioni API; non è stata stabilita la riuscita di questi comandi. Eseguirli esclusivamente in una sessione annidata di prova. In `.github/workflows/linux-verify.yml` è presente una pipeline ripetibile che non finge un esito positivo in caso di compilazione fallita.

## Soglia per definire raggiunta la milestone Wayland

1. `cargo check --workspace --all-targets` completa senza errori.
2. `cargo test --workspace` completa senza errori.
3. Il compositore parte in modalità annidata e accetta un'app GTK4 reale.
4. Focus, popup, move, resize, chiusura e taskbar funzionano sotto test interattivo.
5. DnD file tra due applicazioni GTK4 reali e applicazioni Qt è verificato senza perdita dati.
6. Screenshot della **sessione Wayland reale** registrato separatamente dal test Tk/X11.

Fino ad allora il progetto resta **pre-alpha**.
