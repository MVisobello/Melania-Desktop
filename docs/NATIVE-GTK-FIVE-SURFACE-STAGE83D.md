# Stage 83D — contratto geometrico delle cinque superfici GTK

Il vero `melania-panel` costruisce cinque `ApplicationWindow` layer-shell:
barra (`Top`), Start, impostazioni rapide, notifiche e calendario
(`Overlay`). Il nuovo test Rust verifica sul modello Smithay l'assetto
effettivo di tutte e cinque su un output virtuale 1280×800.

- La barra si estende per 1280×52 nella parte inferiore e riserva 52px.
- Start è 510×565, ancorato a sinistra e in basso, con margini 8/6.
- Quick settings è 350×430, ancorato a destra e in basso, margini 12/65.
- Notifiche è 380×440, ancorato a destra e in basso, margini 12/65.
- Calendario è 330×315, ancorato a destra e in basso, margini 12/58.
- I quattro overlay non riservano area di lavoro; le finestre normali
  mantengono l'area 1280×748 sopra la barra.

## Limiti verificati

Il test è una **prova unitaria del modello geometrico**, non una
sessione GTK nel guest. L'attuale bridge QEMU in modalità
`MELANIA_NATIVE_LAYER_PIXELS` limita il numero di layer surfaces
registrate a **due**, e assegna a tutte la dimensione di configure
`1280×52`. Questi comportamenti sono adeguati al precedente client
di test, ma **non** alle cinque finestre GTK di dimensioni diverse.

Prima di avviare il vero pannello, occorre una modalità QEMU isolata
che gestisca più superfici layer-shell con configure per ciascuna,
commit e buffer propri, chiusura/riapertura senza dangling roles,
focus appropriato e prova di pixel QMP. Non alzare semplicemente il
limite senza verificare gli effetti su memoria, KMS e input.

`NATIVE_SESSION_READY=false`.
