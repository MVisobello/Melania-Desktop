# Milestone M2: Drag & Drop nel compositore Melania

Questa milestone dipende da **M1: avvio compositore proprietario Wayland annidato con input e finestre**. Il codice di M1 non è ancora presente nel repository: lo script smallvil è solo una prova upstream. Non presentare M2 come funzionante prima di test sul compositor vero.

## Integrazione Smithay, piano di implementazione
1. Inizializzare `DataDeviceState` e `SelectionHandler` sul `Display` del compositore; registrare `delegate_data_device` per lo stato del server.
2. Seguire lo stato dell'input `Seat` e impostare il corretto `set_data_device_focus` al cambio di focus tastiera.
3. Tradurre la superficie sotto il puntatore in un target *wl_surface* e reindirizzare gli eventi del drag durante `PointerGrab` o `TouchGrab`; servono seriali dell'input validi.
4. Renderizzare la drag icon come superficie dedicata sopra le normali finestre, senza assegnarle focus.
5. Negoziare MIME/action (`copy`, `move`, `ask`, `link` secondo protocolli usati), monitorando `enter`, `motion`, `leave`, `drop`, `cancel` e *finish*. `melania-dnd` contiene soltanto una macchina a stati applicativa per testare l'ordine degli eventi; NON è il protocollo Smithay.
6. Gestire cancellazione se il client si chiude, perde il seat o cambia superficie; nessuna accessibilità incontrollata alla selection.
7. Per XWayland, collegare XDND senza rimuovere il percorso Wayland-to-Wayland.
8. Eseguire test con `gtk4-demo`, `melania-files`, altre app GTK/Qt e clients XWayland. Accertare che COPY non faccia delete e MOVE completi solo dopo conferma di destinazione.

## Attenzione alle versioni
`smithay` 0.7.0 e le versioni di sviluppo recenti hanno differenze nella API del sottosistema DnD. Prima di scrivere gli handler compilabili fissare precisamente release e funzionalità disponibili; non mischiare esempi `master` e release pubblicate.

## Rischi
- A differenza del movimento delle finestre, il DnD non è solo rendering: è trasferimento di dati fra processi non necessariamente fidati.
- Un target può rifiutare un MIME; non trasformare il rifiuto in successo.
- Evitare `drag/drop` che facciano I/O sincrono nel thread del compositor.
- Richiedere conferma per delete/move cross-device e fallimenti parziali.
