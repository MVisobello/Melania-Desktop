# Melania compositor: percorso M1 (29/10/2026)

## Stato effettivo

**Il compositore Melania non esiste ancora come binario proprietario.** Il progetto include un percorso riproducibile per testare una *vera* sessione Wayland **annidata**, con `smallvil` (esempio didattico di Smithay). È un banco di prova esterno, non il completamento della milestone M1.

## Prima prova senza rischi

Su Linux con X11 o Wayland già funzionante, installa Rust/Cargo, git, librerie dev Wayland/EGL, xkbcommon e GTK 4; per GPU/headless e versioni specifiche vedi documentazione ufficiale Smithay. Poi:

```bash
./scripts/preflight.sh
./scripts/bootstrap-nested-compositor.sh
```

Lo script scarica **Smithay v0.7.0** in `.build/` e avvia `smallvil -c` con il client GTK di Melania come programma di test. Smallvil è mantenuto dal progetto Smithay, non è KDE, GNOME o Cinnamon. Il codice non viene distribuito nel nostro archivio; durante l'installazione è necessario Internet.

Se il backend di `smallvil` fallisce, consulta l'output e i prerequisiti hardware/driver (EGL/Wayland). **Non** avviare dalla schermata di login né cancellare la sessione preesistente.

## Per passare da prova a nostro compositore

1. Importare/adattare in una directory `melania-compositor/` il bootstrap di protocollo Wayland, backend winit, renderer, loop di input da un esempio Smithay, conservando attribuzioni e licenza upstream.
2. Integrare `melania-core` nell'handling di `xdg_toplevel`, focus e posizionamento.
3. Supportare uno specifico protocollo di shell privilegiata in modo sicuro, poi `wlr-layer-shell` per taskbar e quick settings.
4. Aggiungere gestore finestre stacking reale, resize, massimizzazione, minimizzazione, workspace e test end-to-end.
5. Solo dopo: XWayland, login session, DPI/multimonitor, portal, blur, screenshot, cattura video, stabilità e pacchetti.

**Criterio M1 non ancora superato:** una finestra Wayland avviata in una sessione gestita dal nostro codice deve ricevere correttamente tastiera, mouse, dimensionamento e focus.
