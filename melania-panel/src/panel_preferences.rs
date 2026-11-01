//! Real XFCE-like panel controls for the *Melania* GTK layer shell.
//!
//! Values are persisted by Melania Settings and applied by this running
//! process, not by mutating XFCE/GNOME or the host compositor. The default
//! configuration retains the already approved Windows-like shell geometry.
use gtk::{self, gdk, glib, prelude::*};
use gtk::{ApplicationWindow, Box as GtkBox};
use gtk4_layer_shell::{Edge, LayerShell};
use melania_config::Preferences;
use std::{cell::{Cell,RefCell},rc::Rc,time::Duration};

fn relocate(window:&ApplicationWindow,to_top:bool,offset:i32){
    window.set_anchor(Edge::Top,to_top);
    window.set_anchor(Edge::Bottom,!to_top);
    window.set_margin(Edge::Top,if to_top {offset} else {0});
    window.set_margin(Edge::Bottom,if to_top {0} else {offset});
}

/// A hidden panel remains mapped as a Wayland layer surface, with a 3px
/// exposed handle. Unmapping a layer-shell surface during a GTK/Smithay
/// session can invalidate the role and disconnect the Panel entirely.
fn set_revealed(window:&ApplicationWindow,prefs:&Preferences,revealed:bool) {
    let edge=if prefs.panel_position=="top" {Edge::Top} else {Edge::Bottom};
    let actual_height=window.height().max(i32::from(prefs.panel_height));
    let offset=if prefs.panel_auto_hide && !revealed {
        -(actual_height-3).max(0)
    } else {0};
    window.set_margin(edge,offset);
    window.set_exclusive_zone(if prefs.panel_auto_hide {0} else {i32::from(prefs.panel_height)});
}

fn install_opacity(provider:&gtk::CssProvider,prefs:&Preferences){
    if prefs.panel_opacity==94 || prefs.high_contrast {
        // Unmodified defaults retain all existing Aero/Light/Dark CSS.
        provider.load_from_data("");
        return;
    }
    let alpha=f64::from(prefs.panel_opacity)/100.0;
    provider.load_from_data(&format!(
        ".melania-bar {{background: rgba(7,23,45,{alpha:.3});}}\
         window.melania-theme-light .melania-bar {{background: rgba(238,240,244,{alpha:.3});}}\
         window.melania-theme-dark .melania-bar {{background: rgba(36,39,45,{alpha:.3});}}"
    ));
}
fn apply_preferences(
    panel:&ApplicationWindow,bar:&GtkBox,
    start:&ApplicationWindow,quick:&ApplicationWindow,
    notifications:&ApplicationWindow,calendar:&ApplicationWindow,
    prefs:&Preferences,opacity_provider:&gtk::CssProvider,revealed:bool
) {
    let top=prefs.panel_position=="top";
    // Geometry and exclusive zone must follow the same setting to keep the
    // compositor's usable area consistent with the visible taskbar.
    panel.set_anchor(Edge::Top,top);
    panel.set_anchor(Edge::Bottom,!top);
    panel.set_default_size(0,i32::from(prefs.panel_height));
    // Existing 52px defaults are unchanged; other sizes are a true min-height
    // on the real GTK bar widget, rather than an inert Settings value.
    bar.set_size_request(-1,i32::from(prefs.panel_height).saturating_sub(10));
    set_revealed(panel,prefs,revealed);

    let popup_offset=i32::from(prefs.panel_height)+13;
    // Keep the approved lower-edge offsets, not the incorrect earlier
    // change that left a gap beside the Win7 Start menu.
    relocate(start,top,if top {popup_offset}else{6});
    relocate(quick,top,if top {popup_offset}else{65});
    relocate(notifications,top,if top {popup_offset}else{65});
    relocate(calendar,top,if top {popup_offset}else{58});
    install_opacity(opacity_provider,prefs);
}

pub fn attach_live_preferences(
    panel:&ApplicationWindow,bar:&GtkBox,
    start:&ApplicationWindow,quick:&ApplicationWindow,
    notifications:&ApplicationWindow,calendar:&ApplicationWindow
) {
    let provider=gtk::CssProvider::new();
    if let Some(display)=gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,&provider,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION+1);
    }
    let active=Rc::new(RefCell::new(melania_config::load().unwrap_or_default()));
    let hover=Rc::new(Cell::new(false));
    let panel_open=|start:&ApplicationWindow,quick:&ApplicationWindow,
                    notifications:&ApplicationWindow,calendar:&ApplicationWindow| {
        start.is_visible() || quick.is_visible() ||
        notifications.is_visible() || calendar.is_visible()
    };
    let cfg=active.borrow().clone();
    apply_preferences(panel,bar,start,quick,notifications,calendar,
        &cfg,&provider,!cfg.panel_auto_hide||panel_open(start,quick,notifications,calendar));

    let motion=gtk::EventControllerMotion::new();
    let panel_enter=panel.clone();
    let prefs_enter=active.clone();
    let hovered=hover.clone();
    motion.connect_enter(move |_,_,_| {
        hovered.set(true);
        let prefs=prefs_enter.borrow();
        if prefs.panel_auto_hide {set_revealed(&panel_enter,&prefs,true);}
    });
    let panel_leave=panel.clone();
    let prefs_leave=active.clone();
    let hovered=hover.clone();
    let sm=start.clone();
    let qs=quick.clone();
    let nt=notifications.clone();
    let cal=calendar.clone();
    motion.connect_leave(move |_|{
        hovered.set(false);
        let w=panel_leave.clone();
        let prefs=prefs_leave.clone();
        let hover=hovered.clone();
        let sm=sm.clone();
        let qs=qs.clone();
        let nt=nt.clone();
        let cal=cal.clone();
        glib::timeout_add_local_once(Duration::from_millis(650),move ||{
            if hover.get() || sm.is_visible() || qs.is_visible() ||
               nt.is_visible() || cal.is_visible() {return;}
            let p=prefs.borrow();
            if p.panel_auto_hide {set_revealed(&w,&p,false);}
        });
    });
    bar.add_controller(motion);
    let panel=panel.clone();let bar=bar.clone();
    let start=start.clone();let quick=quick.clone();
    let notifications=notifications.clone();let calendar=calendar.clone();
    glib::timeout_add_local(Duration::from_millis(500),move ||{
        if let Ok(latest)=melania_config::load(){
            if *active.borrow()!=latest {
                let reveal=!latest.panel_auto_hide || hover.get() ||
                    panel_open(&start,&quick,&notifications,&calendar);
                apply_preferences(&panel,&bar,&start,&quick,&notifications,&calendar,
                    &latest,&provider,reveal);
                *active.borrow_mut()=latest;
            } else if active.borrow().panel_auto_hide {
                // Also re-hide after transient Start/calendar popovers close.
                let showing=hover.get()||
                    panel_open(&start,&quick,&notifications,&calendar);
                set_revealed(&panel,&active.borrow(),showing);
            }
        }
        glib::ControlFlow::Continue
    });
}
