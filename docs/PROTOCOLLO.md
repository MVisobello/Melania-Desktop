# Protocollo proposto compositore ↔ shell (non implementato)

La shell GTK non deve poter leggere lo stato di finestre di altre applicazioni come un client Wayland qualsiasi. Per taskbar e window management servirà un'estensione privilegiata non esposta indiscriminatamente.

## Eventi (schema concettuale)
- `ToplevelAdded { id: u64, title, app_id, workspace }`
- `ToplevelChanged { id, title?, focused?, minimized?, maximized? }`
- `ToplevelRemoved { id }`
- `WorkspaceChanged { id }`
- `OutputChanged { name, width, height, scale }`

## Azioni (schema concettuale)
- `Activate { id, token }`
- `Close { id }`
- `Minimize { id, desired_state }`
- `MoveToWorkspace { id, workspace_id }`
- `ShowDesktop { enabled }`

L'identità del client shell deve essere autenticata tramite sessione e credenziali su canale protetto. Titoli delle finestre e input sono dati sensibili. Cambi di protocollo versionati, risposte a errori chiuse/finestre non più valide, gestione di riavvio shell e reconnect.
