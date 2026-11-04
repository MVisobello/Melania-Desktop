# Stage 90 — scroll reale mouse e touchpad verso i client Wayland

## Modifica del bridge nativo

Il percorso di input Smithay/libinput QEMU non scarta più
`InputEvent::PointerAxis`. Per ogni scroll valido:

- verifica il dispositivo fisico nel registro della seat attiva e il guard
  generazionale Stage89; nessun input da una VT sospesa;
- seleziona il client Wayland reale sotto il puntatore (hit-test della scena);
- inoltra gli assi orizzontale/verticale e la sorgente con `AxisFrame`,
  conservando i valori v120 della rotellina e i delta continui del touchpad;
- rifiuta valori non finiti/fuori limite prima di inviarli ai client;
- termina l'evento con `wl_pointer.frame`. Non registra il testo digitato.

Il comportamento di clic, trascinamento e tasto Super già verificato rimane
invariato. I test Rust dei limiti v120 e i test di collegamento del bridge
sono aggiunti alla build QEMU.

## Validazione e limiti

Le verifiche automatiche richiedono la compilazione Rust del codice reale.
**Non** rappresentano ancora una prova end-to-end che lo scroll faccia
muovere una lista GTK o Qt tramite una macchina virtuale: questa richiede
l'iniezione di un vero evento USB wheel in QEMU e una ricevuta dell'app
Wayland, senza usare input sintetico direttamente nel client.

Il bridge nativo continua a essere riservato alla VM, con TTY dedicata e
flag espliciti. Non è installabile come sessione di login:
`NATIVE_SESSION_READY=false`.
