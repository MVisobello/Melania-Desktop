# Roadmap Melania Desktop — aggiornata 0.9

**Stato:** sorgenti pre-alpha, non installare come desktop principale.

- **M7 / 0.9** — consolidamento trasferimenti, copia e spostamento espliciti, controllo IPC, test.
- **M8 (bloccante)** — compilazione del workspace e correzione errori delle API GTK/Smithay.
- **M9 (bloccante)** — sessione Wayland annidata con finestre, focus, popup e resize verificati.
- **M10** — drag & drop end-to-end file → file manager, file → client GTK/Qt, hover in taskbar, MOVE sicuro.
- **M11** — XWayland, clipboard, portali XDG, multi-monitor, HiDPI e file manager asincrono.
- **M12** — Aero Glass, anteprime, gesture, accessibilità, login e packaging.

Non promettiamo la v1.0 prima dei test M8–M11.

## Versione 0.18 — Desktop Surface (30 ottobre 2026)
- [x] Sorgenti GTK 4/Layer Shell della superficie desktop indipendente su layer Background
- [x] Lettura cartella XDG Desktop, apertura file e sorgenti `gdk::FileList`
- [x] Gestore di drop COPY-only con `melania-transfer`; nessuna cancellazione automatica
- [x] Monitor delle modifiche alla cartella e aggiornamento icone
- [x] Wallpaper originale e variabile `MELANIA_WALLPAPER`
- [x] Inclusione nel Cargo workspace, nella CI e nel launcher annidato
- [x] 34 controlli statici Python eseguiti con successo
- [ ] **Compilazione reale Rust + GTK 4 + Smithay (non disponibile in questo ambiente)**
- [ ] **Sessione Wayland annidata verificata con desktop, taskbar e Files**
- [ ] Copia e MOVE negoziato fra applicazioni / XWayland, con test di regressione e preservazione dati
- [ ] Blur Aero reale nel renderer del compositore e anteprime della taskbar

## Versione 0.19 — Interazione desktop (30 ottobre 2026)
- [x] Sorgenti GTK per selezione con clic / Ctrl-clic e doppio clic per aprire
- [x] DnD multi-file COPY-only sulle icone delle cartelle e sulla Scrivania
- [x] Menu contestuale desktop e nuova cartella con create_dir atomica
- [x] `melania-desktop-core` isolato da GTK, unit test Rust predisposti
- [x] Scorciatoie XDG del menu Start localizzate
- [x] 41 controlli strutturali Python e 5 test filesystem del prototipo passati
- [ ] **Cargo: compilazione e test Rust** (non disponibili nel container)
- [ ] **Sessione Wayland/GTK realmente avviata**
- [ ] MOVE fra client, drag hover taskbar e XWayland collaudati
