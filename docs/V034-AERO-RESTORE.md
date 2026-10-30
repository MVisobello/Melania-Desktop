# Melania v0.34 — Regressione Aero corretta

## Perché questa versione
La v0.33 aveva rimosso il titolo separato di Melania Files spostando le schede in alto,
ma la resa della barra e degli elementi della shell appariva più piatta e scura del concept.

## Interventi verificabili nel laboratorio X11
- Schede direttamente nella parte alta della finestra, senza titolo sopra.
- Cornici color vetro, blur reale per campionamento dello sfondo via Pillow,
  riflessi superiori, bordi chiari e ombra soffice.
- Taskbar e menu Start alleggeriti (trasparenza, sfocatura e highlight).
- Sfondo astratto originale conservato; nuova versione luminosa aggiunta come
  `UX-Lab/assets/wallpaper-aero.png`.
- Icon pack ufficiale Melania Aero Classic invariato.
- Nessuna regressione rilevata nei tre smoke test delle schede.

## Come eseguire la demo
Dalla directory UX-Lab: `python melania_aero_live.py`.
Richiede Python 3, Pillow, Tk e una sessione X11. Non modifica il desktop host.

## Stato reale
Gli screenshot in `Screenshots-v034/` vengono dall'app Tk/X11 eseguita su Xvfb.
Non costituiscono una compilazione né un avvio del desktop Rust/GTK/Wayland.
Nel progetto GTK è stata aggiornata la palette CSS delle tab, ma non verificata
la presenza di un vero headerbar CSD nel compositor né la sfocatura GPU.
Un wallpaper fotografico completo come nel concept deve essere fornito in futuro:
non si è interpolato uno screenshot pieno di finestre spacciandolo per fotografia.
