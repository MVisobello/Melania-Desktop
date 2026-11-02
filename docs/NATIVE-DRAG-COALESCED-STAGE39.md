# Stage 39 — ridisegno coalescente durante un drag (QEMU)

Due veri movimenti di mouse, sempre con il pulsante sinistro premuto:
1. finestra B: da (650,190) a (740,270); KMS presenta frame 4.
2. mentre il frame 4 è ancora in volo, il mouse sposta B a (800,310).

Il `RenderScheduler` non avvia un altro frame mentre KMS non ha
confermato quello in corso. Il compositore conserva invece l'ultima
generazione della scena. Dopo l'ACK di frame 4 il bridge confronta
`layout_generation` e `presented_layout_generation`; solo in caso
di differenza avvia frame 5, conservando i pixel dei due client.

Un ulteriore screenshot diretto QMP del display virtuale verifica:
la finestra A invariata, la vecchia posizione di B ripulita, il
bordo B e le due metà colorate alla nuova posizione. Sono richiesti
anche ACK 4 e 5 distinti e il ripristino del CRTC al termine.

Il test è **intenzionalmente limitato a 5 frame**: prova un successore
coalescente, non un loop di rendering senza limiti. Non significa
ancora supporto nativo host, produzione o resize Wayland completo.
La transazione `xdg_toplevel.configure` e l'aggiornamento delle
dimensioni dei buffer client restano attività successive.
