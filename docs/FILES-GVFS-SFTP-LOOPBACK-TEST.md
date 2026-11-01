# Test GVfs SFTP autenticato su server temporaneo

Questa verifica è più realistica della modalità `file:///`: avvia un **server OpenSSH/SFTP reale limitato a 127.0.0.1**, crea chiavi Ed25519 temporanee, effettua l'autenticazione con la chiave, apre una sessione D-Bus privata e monta l'URI con il backend GVfs.

Il test Rust `live_gvfs_sftp_loopback_fixture` legge una directory SFTP, verifica nome e dimensione di un file, copia un file in una cartella temporanea e controlla che **una seconda copia non sovrascriva** la destinazione. Nessun server esterno e nessuna password reale vengono usati; i dati di test sono eliminati alla fine dell'esecuzione GitHub Actions.

La prova viene eseguita dal workflow `.github/workflows/files-gvfs-sftp.yml`, che installa GVfs e il server SSH solo sul runner temporaneo. Lo script `scripts/smoke-gvfs-sftp-loopback.sh` rifiuta di partire fuori da GitHub Actions.

## Limiti

Il test verifica **SFTP con chiave SSH su loopback**, non la compatibilità con tutti i server SFTP, SMB o WebDAV, né le richieste di password e i dialoghi grafici su desktop reale. La compatibilità con server autentici e la parità Nautilus richiedono ulteriori prove documentate nella issue #78.
