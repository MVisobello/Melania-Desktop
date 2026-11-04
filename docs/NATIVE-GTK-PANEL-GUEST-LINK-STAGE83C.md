# Stage 83C — verifica del binario GTK nel guest QEMU reale

## Cosa viene verificato

Il workflow Stage83B costruisce `melania-panel` dal codice Rust reale e
gtk4-layer-shell 1.1.1 dalla sorgente ufficiale al commit verificato
`4867d7b85cdf1e829fc1fd6f1d5f04c42cc99389`.

Stage83C trasferisce **il binario reale** e la libreria ELF risolta
`libgtk4-layer-shell.so.0` in un'immagine ISO9660 in sola lettura. Ubuntu
24.04 in un guest QEMU usa le proprie librerie GTK4 (`libgtk-4-1`).
Il guest verifica con il caricatore dinamico via `ldd` che:

- il binario `melania-panel` sia presente ed eseguibile;
- `libgtk-4.so.1` sia risolta dal guest;
- `libgtk4-layer-shell.so.0` sia risolta dalla ISO verificata;
- non ci siano dipendenze `not found`.

Solo dopo questi controlli il guest stampa
`MELANIA_QEMU_GTK_PANEL_ELF_LINK_PASS=true`. La procedura host rifiuta
l'esecuzione se manca quel marker. I test preesistenti KMS e QMP restano
obbligatori.

## Che cosa NON prova

Questo **non avvia ancora `melania-panel` come applicazione GTK**: è
un controllo delle dipendenze dinamiche nel guest, non una prova di
`GtkApplication`, né di disegno dei widget. Il test Stage83A dei pixel
della barra usa ancora un client libwayland separato. Non spacciare
framebuffer QMP sintetici o screenshot del client di prova per screenshot
della GUI Melania.

Prossimo obiettivo Stage83D: avviare in una macchina QEMU isolata
il binario GTK stesso sul server Wayland nativo, consentire le superfici
del pannello/Start/tray con configure reali e catturare i loro pixel con
verifica indipendente. Conservare il blocco sul login e il controllo
originale della GPU fino a dimostrazione reale e ripetibile.

`NATIVE_SESSION_READY=false`.
