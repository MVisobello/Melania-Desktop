//! Operational settings pages retained from the XFCE feature inventory.
//! Their controls live in Melania Settings' main categorized sidebar, not in
//! a separate XFCE catalogue or in placeholder pages.
use gtk::{prelude::*, Stack};
use crate::{xfce_catalog::{self, XFCE_SECTIONS}, xfce_defaults};

pub const LAZY_PAGE_NAMES: [(&str,&str);5]=[
    ("xfce-preferred","Applicazioni preferite"),
    ("xfce-mime","Associazioni MIME"),
    ("xfce-session","Sessione e avvio"),
    ("xfce-panel","Pannello"),
    ("xfce-desktop","Scrivania e sfondo"),
];

pub fn add_pages(stack:&Stack){
    // Every actual XFCE-derived category is immediately navigable, while
    // expensive GIO association queries wait until its first selection.
    for &(key,title) in &LAZY_PAGE_NAMES {
        crate::lazy_pages::register(stack,key,title);
    }
}

pub fn build_page(key:&str)->Option<gtk::Box> {
    match key {
        "xfce-preferred"=>Some(xfce_defaults::preferred_apps_page()),
        "xfce-mime"=>Some(xfce_defaults::mime_page()),
        "xfce-session"=>Some(crate::xfce_autostart::session_page()),
        "xfce-panel"=>Some(crate::xfce_panel::page()),
        "xfce-desktop"=>Some(crate::xfce_desktop::page()),
        _=>None,
    }
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
