# Melania Desktop — Melania Files e pannello (31 ottobre 2026)

## File manager: miglioramenti reali

- Stato vuoto con disegno e messaggio specifici per cartella senza elementi, ricerca senza risultati e filtro Foto senza corrispondenze.
- Errori di lettura delle directory mostrati senza mascherarli come cartelle vuote.
- La testata delle colonne è nascosta quando la lista è vuota e torna quando compaiono elementi.
- La barra laterale può essere mostrata o nascosta tramite pulsante nella toolbar o **Ctrl+B**.
- Tutte le operazioni di file (Copia, Taglia, Incolla, Rinomina, Cestino, schede, ricerca, anteprima) rimangono collegate ai backend già implementati.

## Pannello

- In **menu Start, impostazioni rapide, centro notifiche e calendario** il tasto **Esc** nasconde il popup attivo senza arrestare Melania Panel.
- Nessun cambiamento al tema Fluent approvato, alla decorazione Windows 7 o all'icon pack.

## Verifiche

- `cargo check -p melania-files -p melania-panel --all-targets` tramite GitHub Actions Linux.
- `cargo test` per l'unità di formattazione dei messaggi di cartella vuota in Melania Files.
- `tests/test_files_panel_ux.py` per la struttura di sidebar, overlay e chiusura popup.
- Smoke test Wayland annidato per controllare avvio e interazione dei componenti.

Le schermate catturate sul display virtuale dimostrano l'avvio ma non sono un test automatico dei gesti in ogni possibile dispositivo. Consegna soltanto dopo l'esito della CI.
