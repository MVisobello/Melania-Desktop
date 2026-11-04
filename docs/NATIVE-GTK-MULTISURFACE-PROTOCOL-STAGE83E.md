# Stage 83E — configure nativo per cinque superfici GTK

## Modifica

Il bridge Wayland/Smithay ha ora la modalità addizionale, **solo QEMU**
`MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM`, esplicitamente dipendente
da `MELANIA_NATIVE_LAYER_PIXELS=YES_QEMU_VM` e dal controllo di accesso
guest/VT già richiesto dal binario nativo.

In questa modalità il compositor accetta esclusivamente cinque
`zwlr_layer_surface_v1` riconosciute dalle namespace effettive
del pannello GTK (`melania-panel`, `melania-start`, `melania-quick`,
`melania-notifications`, `melania-calendar`), rispettandone il layer
Top oppure Overlay. I `configure` inviano la dimensione specifica
di ciascuna superficie, anziché imporre 1280×52 a tutte.

La tabella geometrica è isolata nel modulo Rust
`melania-native/src/gtk_layer_geometry.rs`; i test unitari
verificano tutte le dimensioni, le posizioni e l'area riservata.
Il limite di cinque ruoli è tassativo: namespace sconosciute,
duplicati e layer impropri sono rifiutati con `send_close()`.
La rimozione di un ruolo continua a togliere l'immagine dalla scena
ed invalidare il framebuffer quando necessario.

## Compatibilità

Se la nuova variabile non è impostata, il collaudo QEMU precedente
conserva il limite **due** e la configure **1280×52**; non cambia
il percorso GTK annidato esistente, le icone o l'interfaccia utente.

## Limiti della prova

Questa patch **non avvia il vero `melania-panel`**, non fa affermazioni
su screenshot GTK, focus, tasti Start o stabilità della sessione.
Il nuovo ramo è sottoposto a Rust/Linux/QEMU CI; il QEMU smoke precedente
deve continuare a superare tutti i test. Il passo seguente sarà
un test protocollare dedicato che crea tutte e cinque le superfici
dal guest QEMU e osserva cinque `configure/ack` distinti, prima
dell'esecuzione di GTK e della verifica dei pixel reali.

`NATIVE_SESSION_READY=false`.
