# Stage 38 — quarto frame KMS generato dal trascinamento reale

L'esperimento QEMU crea due vere finestre Wayland, riceve tre commit
`wl_shm` distinti e li mostra sulla GPU virtuale tramite DRM/KMS.
**Dopo** il terzo ACK, un client QMP invia un vero drag con il mouse
virtuale della finestra B, trascinandola dalla fascia superiore
`(760,200)` a `(850,280)`, per spostarla da `(650,190)` a
`(740,270)`.

Il bridge Smithay usa l'input libinput per variare la geometria della
scena già trattenuta. Se la generazione della scena è nuova, invia
**un quarto frame** tramite il worker KMS asincrono, *senza alcun quarto
commit del client*. Come per gli altri frame, il ticket è confermato
solo dopo l'ACK verificato dal renderer DRM-rs sul CRTC.

Il test acquisisce da QMP lo screenshot PPM del **vero framebuffer
QEMU** durante la quarta presentazione. Non basta un marker:
il verificatore controlla i colori della finestra A rimasta invariata,
la vecchia posizione di B tornata background, il bordo e due aree
della finestra B nella **nuova posizione**. Se il test grafico o il
ripristino del CRTC non passa, CI fallisce.

## Limiti

- Non è ancora un compositor continuo. Questo test valida **un singolo
  frame di spostamento reale** oltre ai tre frame del client.
- Non vi è ancora ridimensionamento effettivo: serve la transazione
  `xdg_toplevel.configure` / `ack_configure` / nuovo buffer.
- La GPU host non è mai utilizzata. La funzione richiede il consenso
  QEMU, il TTY dedicato e `MELANIA_NATIVE_DRAG_REDRAW=YES_QEMU_VM`.
- Nessuna modifica al flag `MELANIA_NATIVE_SESSION_READY=false`.

Il passo successivo è sostituire il limite di quattro frame con un
presentatore live che riaccoda solo il *damage* più recente dopo l'ACK,
senza saturare la coda e senza inviare callback Wayland premature.
