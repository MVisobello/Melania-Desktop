# Melania 0.15 — Rapporto dei test

## Eseguiti in questa sessione
- 15 test **Python strutturali**: PASS (non sono test di compilazione Rust).
- 5 test **Python sul filesystem del prototipo X11**: PASS.
- Prototipo Tkinter/X11 su Xvfb: PASS per simulazione del drag & drop con un vero movimento del mouse, verifica del contenuto copiato e del file originale, toggle dimostrativo.

## Non eseguiti
- `cargo check` / `cargo test`: Cargo e Rust assenti dal contenitore.
- Avvio reale delle finestre GTK 4 con la nuova clipboard e con Cestino: librerie GTK 4 di sviluppo assenti.
- Compositore Smithay, Wayland, XWayland, Qt, multi-monitor, Flatpak: non verificati.

**La prova del prototipo X11 preesistente NON testa le nuove funzionalità Rust/GTK**. Il test GTK e il test del compositore rimangono gate bloccanti per un rilascio installabile.
