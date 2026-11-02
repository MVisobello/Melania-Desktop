//! Stable GTK Stack entries and one-shot, on-demand category initialization.
//! Keeping the original stack children prevents broken deep links or empty
//! destinations while slow GIO/DBus-backed settings are still loading.
use gtk::{prelude::*, Box as GtkBox, Label, Orientation, Stack};

pub fn register(stack:&Stack,key:&str,title:&str){
    let placeholder=GtkBox::new(Orientation::Vertical,8);
    placeholder.add_css_class("settings-body");
    let status=Label::new(Some("Caricamento impostazioni…"));
    status.add_css_class("settings-row-description");
    status.set_xalign(0.0);
    placeholder.append(&status);
    stack.add_titled(&placeholder,Some(key),title);
}

/// Retain the registered stack child (name, title, order, navigation links);
/// replace only its temporary contents with the real functional controls.
pub fn fill(stack:&Stack,key:&str,content:GtkBox){
    let Some(host)=stack.child_by_name(key).and_then(|w|w.downcast::<GtkBox>().ok())
        else {return;};
    while let Some(child)=host.first_child(){host.remove(&child);}
    host.append(&content);
}
