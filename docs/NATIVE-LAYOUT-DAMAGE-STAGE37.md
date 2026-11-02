# Stage 37: tracciamento del damage di layout

La scena nativa conserva ora una generazione monotona di cambiamento,
`layout_generation()`, aggiornata quando si modifica realmente il contenuto,
la posizione, lo z-order o il numero delle finestre. Non cambia per un
movimento senza variazione di coordinate, un raise della finestra già in cima
o una rimozione inesistente.

Il contatore è il primo prerequisito per il rendering durante il drag:
un futuro presentatore KMS potrà confrontare la generazione composta con
quella confermata dall'ACK, accodando soltanto il frame più recente e
preservando il limite di un frame in volo. Non equivale ancora a un redraw
KMS: nessun frame addizionale viene generato in questo Stage.

Per il resize effettivo resta necessaria la negoziazione
`xdg_toplevel.configure` con ACK e successivo buffer client dimensionato
correttamente. Il backend resta QEMU-only e non autorizza login nativo.
