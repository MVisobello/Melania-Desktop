//! XFCE 4.20 settings parity inventory, independent of xfconf.
//! Every target is visible and honest about whether functionality is wired.
//! Desktop behavior remains owned by Melania's compositor, never host XFCE.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct XfceSection {
    pub slug: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub available: &'static str,
    pub missing: &'static str,
    pub melania_target: &'static str,
}

pub const XFCE_SECTIONS: [XfceSection; 24] = [
    XfceSection { slug: "accessibility", title: "Accessibilità", icon: "preferences-desktop-accessibility", description: "Tastiera lenta, tasti appiccicosi, ripetizione, ingrandimento e contrasto", available: "Riduzione del movimento e contrasto dell'interfaccia Melania", missing: "Tasti lenti/rimbalzanti, emulazione mouse, zoom del desktop e lettore schermo", melania_target: "accessibility" },
    XfceSection { slug: "appearance", title: "Aspetto", icon: "preferences-desktop-theme", description: "Stile GTK, icone, caratteri, rendering e comportamento dell'interfaccia", available: "Temi Light/Dark/Aero, scelta del pacchetto icone Melania", missing: "Font di sistema, subpixel/antialiasing, temi esterni e scaling globale", melania_target: "appearance" },
    XfceSection { slug: "desktop", title: "Scrivania", icon: "user-desktop", description: "Sfondi, icone desktop, menu contestuale, combinazioni e cambio sfondo", available: "Comportamento delle finestre e preferenze desktop esistenti", missing: "Sfondi per monitor/spazio, disposizione icone, menu desktop e wallpaper cycling", melania_target: "desktop" },
    XfceSection { slug: "display", title: "Schermo", icon: "video-display", description: "Risoluzione, frequenza, rotazione, riflessione, schermo principale, scala e profili", available: "Rilevamento monitor e sensori nella sessione corrente", missing: "Applicazione configurazione monitor, profili e ripristino dopo timeout", melania_target: "display" },
    XfceSection { slug: "file-manager", title: "Gestore file", icon: "system-file-manager", description: "Comportamento visualizzazione, viste, miniature, Cestino e volumi", available: "Melania Files dispone di viste, ricerca, trasferimenti e Cestino", missing: "Preferenze persistenti di Files, colonne, miniature e supporti remoti", melania_target: "storage" },
    XfceSection { slug: "keyboard", title: "Tastiera", icon: "input-keyboard", description: "Layout, ripetizione caratteri, velocità, mappature e scorciatoie applicazioni", available: "Elenco scorciatoie attive di Melania", missing: "Modifica combinazioni, layout multipli, ripetizione e composizione", melania_target: "shortcuts" },
    XfceSection { slug: "mouse", title: "Mouse e touchpad", icon: "input-mouse", description: "Accelerazione, sensibilità, mano primaria, scorrimento e comportamento touchpad", available: "Informazioni sui dispositivi di input Linux", missing: "Configurazione libinput per dispositivo, tap, scorrimento naturale e gesture", melania_target: "devices" },
    XfceSection { slug: "panel", title: "Pannello", icon: "preferences-desktop-toolbar", description: "Posizione, altezza, visibilità, trasparenza, plugin, launcher e taskbar", available: "Preferiti delle applicazioni e raggruppamento delle finestre", missing: "Configurazione posizione/altezza, nascondimento, più pannelli e plugin", melania_target: "desktop" },
    XfceSection { slug: "power", title: "Alimentazione", icon: "battery", description: "Sospensione, schermo inattivo, coperchio, alimentazione e luminosità", available: "Stato reale della batteria", missing: "Profili energetici, azioni coperchio, luminosità e inattività", melania_target: "power" },
    XfceSection { slug: "preferred", title: "Applicazioni preferite", icon: "applications-default", description: "Browser, posta, gestore file e terminale predefiniti", available: "Associazioni XDG reali per browser e posta elettronica", missing: "Selezione del terminale e gestore file predefiniti nella sessione Melania", melania_target: "xfce-preferred" },
    XfceSection { slug: "mime", title: "Associazioni MIME", icon: "text-x-generic", description: "Applicazioni associate ai tipi di file e protocolli", available: "Scelta dell'app predefinita tramite GIO/XDG per MIME selezionati", missing: "Editor avanzato di tutte le associazioni, azioni personalizzate e ripristino", melania_target: "xfce-mime" },
    XfceSection { slug: "removable", title: "Supporti rimovibili", icon: "drive-removable-media", description: "Montaggio automatico, fotocamere, supporti audio e azioni per periferica", available: "Informazioni sui dispositivi da Linux", missing: "Regole autorun, automount e periferiche GVfs/udisks2", melania_target: "devices" },
    XfceSection { slug: "session", title: "Sessione e avvio", icon: "system-run", description: "Avvio automatico applicazioni, sessioni salvate, logout e servizi", available: "Avvio del desktop annidato e gestione base della sessione", missing: "Gestione autostart, servizi e ripristino automatico della sessione", melania_target: "system" },
    XfceSection { slug: "settings-editor", title: "Editor impostazioni", icon: "preferences-system", description: "Vista delle chiavi di configurazione e modifica con controllo dei tipi", available: "Preferenze Melania persistite in formato locale verificabile", missing: "Editor di tutte le chiavi con validazione e ripristino", melania_target: "system" },
    XfceSection { slug: "window-manager", title: "Gestore finestre", icon: "preferences-system-windows", description: "Decorazioni, focus, titolo, pulsanti, tastiera e comportamento finestre", available: "Comandi base, Snap e anteprime barra", missing: "Focus policy, placement, decorazioni per tema e scorciatoie editabili", melania_target: "desktop" },
    XfceSection { slug: "wm-tweaks", title: "Regolazioni finestre", icon: "preferences-system-windows", description: "Compositing, Alt+Tab, ombre, trasparenze, focus e spostamenti", available: "Temi Aero, Snap, raggruppamento e anteprime", missing: "Regolazioni avanzate di focus, compositing e tiling", melania_target: "desktop" },
    XfceSection { slug: "workspaces", title: "Spazi di lavoro", icon: "view-grid", description: "Numero di workspace, nomi, margini, posizione e passaggio", available: "Numero di workspace configurabile (1–8)", missing: "Rinomina, margini, cambio dinamico e spostamento per monitor", melania_target: "workspaces" },
    XfceSection { slug: "notifications", title: "Notifiche", icon: "preferences-system-notifications", description: "Posizione, durata, opacità, anteprima, temi e non disturbare", available: "Non disturbare del sistema notifiche Melania", missing: "Regole per applicazione, cronologia e personalizzazione durata/posizione", melania_target: "notifications" },
    XfceSection { slug: "screensaver", title: "Salvaschermo e blocco", icon: "system-lock-screen", description: "Tempo di inattività, blocco, salvaschermo e sicurezza", available: "Nessuna gestione di blocco dedicata nel compositor", missing: "Blocco schermo reale, inattività e protezione al resume", melania_target: "privacy" },
    XfceSection { slug: "printing", title: "Stampanti", icon: "printer", description: "Gestione stampanti CUPS, code, predefinita e proprietà", available: "Visibilità del supporto stampa Linux tramite applicazioni esterne", missing: "Pannello CUPS integrato e gestione autorizzazioni", melania_target: "devices" },
    XfceSection { slug: "color", title: "Profili colore", icon: "preferences-color", description: "Profili ICC per monitor e calibrazione", available: "Nessun profilo colore applicato dal compositor", missing: "Gestione colord/ICC e trasformazioni colore per output", melania_target: "display" },
    XfceSection { slug: "sound", title: "Audio", icon: "audio-card", description: "Uscita, ingresso, volume e dispositivi di riproduzione", available: "Volume di uscita reale tramite PipeWire/WirePlumber", missing: "Selezione dispositivi, ingresso, bilanciamento e stream per-app", melania_target: "audio" },
    XfceSection { slug: "network", title: "Rete", icon: "network-workgroup", description: "Connessioni cablate, Wi-Fi, VPN e stato", available: "Stato NetworkManager e aggiornamento su richiesta", missing: "Editor connessioni e profili Wi-Fi/VPN integrato", melania_target: "network" },
    XfceSection { slug: "bluetooth", title: "Bluetooth", icon: "bluetooth", description: "Controller, associazione, connessione e periferiche", available: "Informazioni BlueZ sul controller", missing: "Associazione/disconnessione dispositivi via D-Bus", melania_target: "bluetooth" },
];

pub fn has_page(page:&str)->bool {
    page == "xfce" || XFCE_SECTIONS.iter().any(|section| {
        page.strip_prefix("xfce-")==Some(section.slug)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustive_catalog_has_unique_pages_and_working_targets() {
        use std::collections::HashSet;
        let mut keys=HashSet::new();
        for entry in XFCE_SECTIONS {
            assert!(keys.insert(entry.slug));
            assert!(!entry.title.is_empty());
            assert!(!entry.description.is_empty());
            assert!(!entry.available.is_empty());
            assert!(!entry.missing.is_empty());
            assert!(has_page(&format!("xfce-{}",entry.slug)));
        }
        assert!(!has_page("xfce-../../files"));
        assert!(!has_page("xfce-unimplemented"));
    }
}
