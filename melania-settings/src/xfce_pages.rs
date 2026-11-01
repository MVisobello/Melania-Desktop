//! Catalog of XFCE 4.20 Settings inside Melania's own GTK4 Settings shell.
//! Borrow *feature coverage*, not XFCE's window layout or xfconf backend.
//! Every section is discoverable and labels absent functionality explicitly.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Label, Orientation, Stack};
use crate::{gnome_pages, xfce_catalog::{self, XFCE_SECTIONS}, xfce_defaults};

fn label(value:&str,class:&str)->Label {
    let label=Label::new(Some(value));
    label.add_css_class(class);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}
fn page(title:&str,subtitle:&str)->GtkBox {
    let body=GtkBox::new(Orientation::Vertical,9);
    body.add_css_class("settings-body");
    body.append(&label(title,"settings-heading"));
    body.append(&label(subtitle,"settings-subtitle"));
    body
}
pub fn add_pages(stack:&Stack) {
    let overview=page("Impostazioni complete · riferimento XFCE",
        "Tutte le aree di configurazione di XFCE 4.20, adattate all'architettura indipendente di Melania.");
    overview.append(&label(
        "La copertura è progressiva: le funzioni già operative aprono controlli reali, "
        ,"settings-row-description"));
    overview.append(&label(
        "quelle ancora da sviluppare sono identificate chiaramente. L'interfaccia resta quella di Melania.",
        "settings-row-description"));
    let container=GtkBox::new(Orientation::Vertical,0);
    container.add_css_class("settings-group-card");
    for spec in XFCE_SECTIONS {
        let target=format!("xfce-{}",spec.slug);
        let description=format!("{} · {}",spec.description,spec.available);
        let row=gnome_pages::linked_row(stack,&target,spec.icon,spec.title,&description);
        container.append(&row);
    }
    overview.append(&container);
    stack.add_titled(&overview,Some("xfce"),"Completezza XFCE");

    for spec in XFCE_SECTIONS {
        let target=format!("xfce-{}",spec.slug);
        // Real default-browser/mail/MIME controls, not a mockup.
        let body=match spec.slug {
            "preferred" => xfce_defaults::preferred_apps_page(),
            "mime" => xfce_defaults::mime_page(),
            "session" => crate::xfce_autostart::session_page(),
            "panel" => crate::xfce_panel::page(),
            "desktop" => crate::xfce_desktop::page(),
            _ => {
                let body=page(spec.title,spec.description);
                body.append(&label("Funzioni disponibili","settings-section-heading"));
                body.append(&gnome_pages::info_row("emblem-default",spec.title,spec.available));
                body.append(&label("Da completare per la piena compatibilità funzionale",
                    "settings-section-heading"));
                body.append(&gnome_pages::info_row("dialog-information",
                    "Non ancora operativo",spec.missing));
                let destination=spec.melania_target;
                if destination!="xfce-preferred" && destination!="xfce-mime" {
                    let name=format!("Apri impostazioni Melania · {}",spec.title);
                    body.append(&gnome_pages::linked_row(
                        stack,destination,spec.icon,&name,
                        "Vai alle opzioni effettivamente disponibili; nessun interruttore fittizio."
                    ));
                }
                body
            }
        };
        stack.add_titled(&body,Some(&target),spec.title);
    }
}
pub fn is_known_page(page:&str)->bool {xfce_catalog::has_page(page)}
