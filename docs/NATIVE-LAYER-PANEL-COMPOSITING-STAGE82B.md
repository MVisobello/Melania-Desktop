# Native Stage 82B — compositing in ordine di livello

Questa fase collega il modello layer-shell della Stage 82A al renderer
CPU gia presente in melania-native. Il retained scene supporta superfici
associate a background, bottom, normal, top e overlay. L'interfaccia di
inserimento esistente continua a creare finestre di livello normal.

Il rendering completo e quello a rettangoli danneggiati usano lo stesso
ordine di livelli del compositore CPU. Anche la selezione della superficie
sotto il puntatore rispetta la priorita dei livelli, e non soltanto
l'ordine temporale di apertura delle finestre.

Tre nuovi test Rust controllano una taskbar davanti a una finestra
aperta successivamente, il cambio di livello con cancellazione del
danno solo dopo un ACK del generatore, e la corretta composizione alfa
premoltiplicata di un overlay.

La PR NON importa ancora i veri pixel wl_shm di WlrLayerSurface, NON
crea una sessione GTK nativa, NON cambia la grafica e NON abilita il
login autonomo. Questi collegamenti richiedono test guest QEMU propri.
NATIVE_SESSION_READY=false.
