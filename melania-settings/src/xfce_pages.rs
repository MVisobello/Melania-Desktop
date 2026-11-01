//! Operational settings pages retained from the XFCE feature inventory.
//! Their controls live in Melania Settings' main categorized sidebar, not in
//! a separate XFCE catalogue or in placeholder pages.
use gtk::{prelude::*, Stack};
use crate::{xfce_catalog::{self, XFCE_SECTIONS}, xfce_defaults};

pub fn add_pages(stack: &Stack) {
    // Only create pages that actually expose Melania controls. All five are
    // registered in SETTINGS_NAVIGATION and therefore always discoverable.
    stack.add_titled(&xfce_defaults::preferred_apps_page(),
        Some("xfce-preferred"), "Applicazioni preferite");
    stack.add_titled(&xfce_defaults::mime_page(),
        Some("xfce-mime"), "Associazioni MIME");
    stack.add_titled(&crate::xfce_autostart::session_page(),
        Some("xfce-session"), "Sessione e avvio");
    stack.add_titled(&crate::xfce_panel::page(),
        Some("xfce-panel"), "Pannello");
    stack.add_titled(&crate::xfce_desktop::page(),
        Some("xfce-desktop"), "Scrivania e sfondo");
}

/// Maintain compatibility with old "xfce-*" command-line/deep links. Generic
/// catalogue entries navigate to the corresponding real Melania category.
/// Do not recreate dead duplicate pages or pretend unimplemented controls exist.
pub fn canonical_page(page: &str) -> &str {
    match page {
        "xfce" => "system",
        "xfce-preferred" | "xfce-mime" | "xfce-session" |
        "xfce-panel" | "xfce-desktop" => page,
        _ => XFCE_SECTIONS.iter()
            .find(|spec| page.strip_prefix("xfce-") == Some(spec.slug))
            .map(|spec| spec.melania_target)
            .unwrap_or(page),
    }
}
pub fn is_known_page(page: &str) -> bool { xfce_catalog::has_page(page) }
