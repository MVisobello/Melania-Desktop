//! Single shared Melania titlebar control implementation for GTK4 windows.
//! Both Files and Settings include this module so the Windows 7-style
//! buttons cannot drift apart in glyphs, sizing or behavior.
use gtk::{prelude::*, ApplicationWindow, Box as GtkBox, Button, HeaderBar, Orientation};

pub fn install(window: &ApplicationWindow, header: &HeaderBar) {
    // Never mix the native GTK windowcontrols with Melania's own buttons.
    header.set_show_title_buttons(false);
    header.set_decoration_layout(Some(""));
    header.add_css_class("melania-win7-header");

    let controls = GtkBox::new(Orientation::Horizontal, 0);
    controls.add_css_class("win7-window-controls");
    // GtkHeaderBar would otherwise stretch the compact buttons to its
    // full titlebar height. Windows 7 buttons are top-aligned and shallow.
    controls.set_valign(gtk::Align::Start);

    let minimize = Button::with_label("—");
    minimize.add_css_class("win7-minimize");
    minimize.set_tooltip_text(Some("Minimizza"));
    let maximize = Button::with_label("□");
    maximize.add_css_class("win7-maximize");
    maximize.set_tooltip_text(Some("Massimizza / Ripristina"));
    let close = Button::with_label("×");
    close.add_css_class("win7-close");
    close.set_tooltip_text(Some("Chiudi"));

    let w = window.clone();
    minimize.connect_clicked(move |_| w.minimize());
    let w = window.clone();
    maximize.connect_clicked(move |_| {
        if w.is_maximized() { w.unmaximize(); } else { w.maximize(); }
    });
    let maximize_label = maximize.clone();
    window.connect_maximized_notify(move |w| {
        maximize_label.set_label(if w.is_maximized() { "❐" } else { "□" });
    });
    let w = window.clone();
    close.connect_clicked(move |_| w.close());

    for button in [&minimize, &maximize, &close] {
        controls.append(button);
    }
    header.pack_end(&controls);
}
