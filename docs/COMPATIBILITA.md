# Compatibilità, privacy e sicurezza

## Integrazione applicativa desiderata

- GTK 2/3/4 e Qt 5/6: usare Wayland quando disponibile, altrimenti XWayland.
- App X11: XWayland richiede implementazione e test sul compositore.
- Wine: funzionamento dipendente da build, driver e compatibilità XWayland/Wayland.
- Flatpak: necessaria implementazione/inclusione `xdg-desktop-portal` appropriata; evitare grant impliciti.
- AppImage e Snap: per app compatibili e secondo supporto distro.
- Drag-and-drop, clipboard e screen sharing: trattare come requisiti essenziali.

## Sicurezza

- Menu e shell senza privilegi root; non usare shell-script intermedi con input utente non sanificato.
- Controlli Wi-Fi/Bluetooth/energia inizialmente tramite comandi dei servizi di sistema senza shell interpolata; in una versione successiva attraverso D-Bus e PolicyKit.
- Clipboard e screenshot: privacy per app sandboxate, niente letture globali indiscriminate.
- Minimizzare telemetria; nessuna raccolta dati di default.
- Salvataggio preferenze in XDG config, secret storage tramite servizi dedicati.

## Debiti tecnici noti del prototipo

- `melania-preview`: demo in una finestra, **non** desktop o pannello reale. Il launcher legge GIO/XDG, ma la taskbar non elenca ancora le finestre del compositor.
- `melania-files`: accessi filesystem sincroni sulla UI (da sostituire con async/thread), ricerca per nome solo nella cartella, miniature non cachate, cartelle XDG non localizzate.
- `melania-settings`: opzioni UI ora persistenti tramite `melania-config`; Aero/finestre non applicati finché manca il compositore. Le impostazioni hardware rapide usano `melania-services`, se disponibili.
- GTK e codice Rust non ancora compilati in un ambiente con toolchain completa.
