# Melania Settings — navigazione compatta e stato rete

Questa revisione prosegue la UI GTK4 già ispirata a GNOME e mantiene **intatti** icon pack Fluent, pulsanti Windows 7, pannello e impostazioni host Linux.

## Finestra stretta

- Il nuovo pulsante `☰` nella `GtkHeaderBar` mostra/nasconde la sidebar: è un vero `GtkRevealer` e non un'immagine.
- `Ctrl+B` attiva lo stesso comportamento anche se il focus è in un controllo delle Impostazioni (scope GTK Managed).
- L'area principale rimane scorrevole e utilizza una larghezza minima meno rigida.
- Il programma non forza la risoluzione del monitor e non cambia configurazioni di GNOME, Cinnamon o altri ambienti desktop.
- Su schermi molto stretti, il pannello laterale può essere nascosto manualmente. **Non** si dichiara di avere un breakpoint automatico ancora non implementato.

## Stato della rete

La pagina `Rete` legge NetworkManager con `nmcli`. Il pulsante **Aggiorna stato della rete** ripete la lettura e aggiorna il testo visibile senza riavviare Settings. Non modifica Wi-Fi, indirizzi IP o connessioni di sistema.

## Test

`tests/test_settings_responsive.py` protegge l'integrazione del toggle, la scorciatoia, i controlli di navigazione e il comando di aggiornamento. La pipeline Linux compila Rust/GTK e `settings-visual.yml` cattura schermate dell'app vera in ambiente virtuale.

Il menu Start di Melania e l'icon pack Fluent originale non sono stati ritoccati in questa revisione.
