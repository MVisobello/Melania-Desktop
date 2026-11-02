# Stage 36 — drag e resize: prima integrazione del modello nativo

Il backend QEMU-only ha ora uno stato di interazione con il puntatore:
`WindowInteraction` crea un grab con ID di finestra e geometria iniziale
solo dopo un clic sinistro reale processato dal bridge libinput/Smithay.

- Una pressione nella fascia superiore di 26 px avvia **Move**.
- Una pressione nell'angolo inferiore destro di 16×16 px avvia
  **ResizeBottomRight**.
- Movimento e resize validano coordinate finite, dimensioni minime,
  output virtuale 1280×800 e un solo grab attivo per volta.
- Il rilascio del pulsante sinistro chiude l'interazione. Una pausa
  della sessione libseat cancella ogni grab rimasto.
- Il trascinamento modifica la geometria della scena `MultiWindowScene`;
  il resize produce, per ora, una **preview nel modello**, senza alterare
  prematuramente dimensioni del client `wl_shm`.

**Limite importante:** la prova QEMU KMS attuale dispone soltanto di
tre aggiornamenti reali originati da un client Wayland. Il movimento
della scena durante un drag non provoca ancora un nuovo frame visibile
sulla GPU; il nuovo rendering avviene solo al successivo commit reale
del client. Il ridimensionamento reale richiederà il corretto protocollo
`xdg_toplevel.configure`, l'ACK e il buffer client della nuova misura.

Questo milestone è quindi **l'integrazione dei gesti e della geometria**,
non la dichiarazione che il drag/resize sia già funzionante su uno schermo
nativo. Il passaggio successivo deve riconciliare damage del layout,
KMS async presenter, callback, input e nuova misura della superficie,
con test QEMU screenshot veri. `NATIVE_SESSION_READY=false` resta obbligatorio.
