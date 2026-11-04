# Stage 83F — cinque `configure/ack` Wayland reali in QEMU

Il test Stage83E del modello Rust è esteso con un **client libwayland
reale**, `qemu-gtk-five-roles`, avviato nel guest QEMU isolato su un
socket Wayland creato dal server Smithay nativo. Per ciascuna delle
namespace `melania-panel`, `melania-start`, `melania-quick`,
`melania-notifications` e `melania-calendar`, il client crea una
`wl_surface` indipendente, richiede la corretta superficie layer-shell
con gli anchor/margini della GUI GTK, riceve `configure` dalle callback
Wayland, ne verifica le dimensioni ed esegue `ack_configure`.

La CI richiede:
- cinque ACK reali dal client (con dimensioni esatte);
- cinque eventi server namespace/dimensioni corrispondenti;
- conteggio lato server pari a cinque;
- tutti i precedenti test QEMU, Rust, Linux e SFTP superati.

Il server QEMU si avvia con `MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM`
e `MELANIA_NATIVE_SHELL_PROBE=YES_QEMU_VM`, in una sessione
**esclusivamente protocollare**. Non è abilitata l'importazione pixel
GTK in questo step; il collaudo DRM/KMS esistente usa la sua modalità
separata. I ruoli non ricevono focus per il solo commit.

## Limite esplicito

**Questo programma non è GTK**: crea le stesse cinque superfici
mediante la vera libreria Wayland, ma non disegna icone, non carica
widget, non apre Start e non fornisce screenshot GTK. La reale app
`melania-panel` nel guest, gli eventi input e i pixel QMP
rimangono un obiettivo distinto. `NATIVE_SESSION_READY=false`.
