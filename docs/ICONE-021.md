# Melania Desktop — Icon Pack v0.21 (source integration)

L'icona pack **Melania Aero Classic v2** è ora incluso in `branding/Melania-Aero-Classic`.
Le applicazioni GTK Melania Files, Melania Panel, Melania Settings e Melania Desktop Surface sono configurate per cercare e usare questo tema **solo nei processi Melania**, senza sostituire automaticamente il tema Cinnamon/GNOME dell'utente.

Il precedente tema `branding/Melania` rimane nel progetto e non è stato eliminato.

Le **icone grandi** sono PNG individuali ricavati dalle tavole approvate; le **icone sidebar simboliche** sono SVG originali in `scalable/`.

**Attenzione:** il workspace Rust/GTK/Smithay resta pre-alpha e non è stato compilato o collaudato su Wayland in questa sessione. La verifica positiva riguarda il tema icone (file, cache GTK, trasparenza, dimensioni, ZIP), non il compositore.

Per installare il tema da solo, usare l'archivio `Melania-Aero-Classic-Icon-Pack-v2.zip` e il relativo `install.sh`.
