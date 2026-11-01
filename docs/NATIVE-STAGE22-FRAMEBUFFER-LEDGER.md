# Stage 22 — ledger per i riferimenti GEM dei framebuffer

La prova QEMU della fase 21 ha acquisito e chiuso veri riferimenti GEM
mediante GETFB2/GEM_CLOSE. Questa fase aggiunge in Rust
`FramebufferHandleLedger` per descrivere esplicitamente le condizioni
di possesso: coppia (DRM fd, framebuffer), handle GEM distinti, assenza
di alias condivisi non gestiti e conferma di chiusura soltanto dopo il
successo delle operazioni kernel.

I test Rust coprono alias multiplanari, handle nulli, doppia acquisizione,
fd diverso, chiusura parziale e obbligo di zero riferimenti pendenti.
Il ledger non apre fd e non esegue ioctl; la verifica reale del kernel
rimane affidata alla prova QEMU Stage 21.

**Non è ancora un ripristino KMS**: non conserva handle fra modeset,
non possiede blob DRM, non esegue rollback atomico, non abilita scanout,
non crea una sessione di login. `visible_kms_commit_allowed()`
continua a restituire false. Il prossimo passo richiede un owner Rust
effettivo delle risorse, test di modeset reversibile e ritorno VT
in VM usa-e-getta.
