# Melania Files + barra icone — implementazione

L'interfaccia segue la schermata di riferimento approvata, senza sostituire
il tema Aero o riscrivere il backend filesystem.

## Melania Files

- Una sola barra superiore GTK4 `HeaderBar`: `+` resta sempre a sinistra,
  poi le schede nell'ordine di apertura, infine i controlli nativi
  minimizza/massimizza/chiudi all'estrema destra.
- La precedente seconda barra delle schede GTK `Notebook` non è visibile:
  `Notebook` continua a gestire gli stati reali delle pagine, spostati in
  un tab strip nella barra titolo. Il label del tab si aggiorna con la
  navigazione esistente.
- I pulsanti `+`, chiudi scheda, Ctrl+T, Ctrl+W e Ctrl+Tab restano collegati
  agli stessi browser indipendenti; l'ultima scheda non viene chiusa
  accidentalmente.
- Toolbar comandi in **una sola riga**: Nuovo, Taglia, Copia, Incolla,
  Rinomina, Elimina, Ordina, Visualizza, Altro.
- Gli strumenti secondari rimangono raggiungibili nei popover e NON sono
  stati eliminati: file vuoto, cartella, modelli, nascosti, solo foto,
  ricerca ricorsiva, preferiti, inverti ordinamento, dettagli, elenco,
  oltre a navigazione, ricerca, proprietà e trasferimenti.
- Una seconda barra compatta sotto i comandi mantiene solo la navigazione
  (indietro, avanti, su, home, aggiorna, percorso, ricerca), evitando di
  mischiare operazioni filesystem e spostamenti fra cartelle.
- Nessun cambio a logica clipboard/DnD, operazioni asincrone, move sicuro,
  cestino recuperabile, rinomina senza sovrascrittura, monitor cartelle.

## Barra applicazioni

- Solo icone anche per le applicazioni fissate; tooltips descrittivi restano.
- Una finestra Melania Files aperta rimpiazza la propria icona fissata, non
  genera il duplicato `File` più `Home`.
- Tutte le finestre Melania Files sono raggruppate nella stessa icona per
  default, anche quando l'opzione facoltativa di raggruppamento generale è
  disattivata.
- Due o più finestre aprono al passaggio del mouse un popover con icona,
  titolo di ciascuna finestra e pulsante per attivarla o chiuderla.
- Il titolo Wayland della finestra Files segue la cartella della scheda attiva,
  così nel selettore si distinguono ad esempio `Documenti — Melania Files`
  e `Immagini — Melania Files`, anziché due voci identiche.
- Gli elementi **scheda** dentro la stessa finestra NON sono finestre
  Wayland indipendenti: rimangono selezionabili dalla barra delle schede.
- **Limite esplicito**: il popover non contiene ancora miniature live dei
  contenuti delle finestre. Occorre implementare un canale di cattura
  controllato in Smithay e un aggiornamento a frequenza limitata in panel.
  Non attribuire a GTK il permesso di catturare altre superfici Wayland.

## Criteri di verifica

- `cargo check -p melania-files -p melania-panel -p melania-compositor`.
- `cargo test -p melania-panel --bin melania-panel` (Files forzatamente
  raggruppato; altre app seguono la preferenza).
- Sessione nested reale e test di DnD.
- Ispezione manuale dell'ordine tab, dei pulsanti finestra e della visibilità
  dei comandi su 1024×768 e 1280×800.
- Testare 2 finestre Files e 2 schede Files distintamente.
- Verificare che chiusura e selezione via IPC siano mirate alla finestra
  corretta; non inventare thumbnail in screenshot dimostrativi.
