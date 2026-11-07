//! Real XFCE-like panel controls for the *Melania* GTK layer shell.
//!
//! Values are persisted by Melania Settings and applied by this running
//! process, not by mutating XFCE/GNOME or the host compositor. The default
//! configuration retains the already approved Windows-like shell geometry.
use gtk::{self, gdk, glib, prelude::*};
use gtk::{ApplicationWindow, Box as GtkBox};
use gtk4_layer_shell::{Edge, LayerShell};
use melania_config::Preferences;
use std::{cell::{Cell,RefCell},fs,os::unix::fs::MetadataExt,path::Path,rc::Rc,time::Duration};

fn config_stamp(path:&Path)->Option<(u64,u64,i64,i64)> {
    fs::metadata(path).ok().map(|meta|
        (meta.ino(),meta.len(),meta.mtime(),meta.mtime_nsec()))
}

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

// GTK layer-shell alpha really exposes the background surface to the host
// compositor. This is glass translucency, NOT a sampled backdrop blur pass.
// Keep the user's panel opacity as a ceiling and honor High Contrast.
fn glass_opacity(prefs:&Preferences)->f64 {
    let requested=f64::from(prefs.panel_opacity)/100.0;
    if prefs.appearance_theme()=="aero" && !prefs.high_contrast {
        let glass=(0.92-f64::from(prefs.blur_strength)*0.0024).clamp(0.68,0.92);
        requested.min(glass)
    } else {
        requested
    }
}

fn install_opacity(provider:&gtk::CssProvider,prefs:&Preferences){
    if prefs.high_contrast {
        provider.load_from_data("");
        return;
    }
    let alpha=glass_opacity(prefs);
    if prefs.appearance_theme()=="aero" {
        // Actual translucent client-owned Wayland pixels. The higher
        // reflectance at the top is the Win7 glass highlight, but no shader
        // samples the scene behind this layer until the native blur pass lands.
        let top=(alpha+0.10).min(0.96);
        provider.load_from_data(&format!(
            "window.melania-theme-aero .melania-bar {{ \
             background: linear-gradient(to bottom, \
             rgba(84,151,215,{top:.3}) 0%, rgba(18,64,112,{alpha:.3}) 48%, \
             rgba(6,31,67,{alpha:.3}) 100%); \
             border-top: 1px solid rgba(245,252,255,0.79); \
             box-shadow: inset 0 1px rgba(255,255,255,0.75), \
             inset 0 8px 18px rgba(221,244,255,0.19), \
             0 -6px 22px rgba(1,12,33,0.30); }} "
        ));
    } else if prefs.panel_opacity==94 {
        // Keep the existing opaque Light/Dark defaults exactly as before.
        provider.load_from_data("");
    } else {
        provider.load_from_data(&format!(
            ".melania-bar {{background: rgba(7,23,45,{alpha:.3});}} \
             window.melania-theme-light .melania-bar {{background: rgba(238,240,244,{alpha:.3});}} \
             window.melania-theme-dark .melania-bar {{background: rgba(36,39,45,{alpha:.3});}}"
        ));
    }
}
fn apply_preferences(
    panel:&ApplicationWindow,bar:&GtkBox,
    start:&ApplicationWindow,quick:&ApplicationWindow,
    notifications:&ApplicationWindow,calendar:&ApplicationWindow,
    prefs:&Preferences,opacity_provider:&gtk::CssProvider,revealed:bool
) {
    // Settings writes are watched live. Update ALL layer-shell windows, not
    // just the bar, so Aero/Light/Dark switches do not require restarting.
    for surface in [panel,start,quick,notifications,calendar] {
        for class in ["melania-theme-aero","melania-theme-light","melania-theme-dark"] {
            surface.remove_css_class(class);
        }
        surface.add_css_class(&format!("melania-theme-{}",prefs.appearance_theme()));
    }
    let top=prefs.panel_position=="top";
    // Geometry and exclusive zone must follow the same setting to keep the
    // compositor's usable area consistent with the visible taskbar.
    panel.set_anchor(Edge::Top,top);
    panel.set_anchor(Edge::Bottom,!top);
    panel.set_default_size(0,i32::from(prefs.panel_height));
    // The top edge must not inherit the bottom bar's vertical padding/minimum:
    // that combination made the 52px panel grow when it was moved upwards.
    // Keep the chosen height identical at both edges, including custom sizes.
    if top { bar.add_css_class("melania-bar-top"); }
    else { bar.remove_css_class("melania-bar-top"); }
    let vertical_padding=if top {4} else {10};
    bar.set_size_request(-1,i32::from(prefs.panel_height).saturating_sub(vertical_padding));
    set_revealed(panel,prefs,revealed);

    // The compositor accounts for the layer's exclusive top zone itself.
    // Adding the entire panel height here would double that offset and leave
    // an 80px gap above Start when the taskbar moves to the upper edge.
    // Preserve all approved bottom-edge popup distances.
    relocate(start,top,6);
    relocate(quick,top,if top {12}else{65});
    relocate(notifications,top,if top {12}else{65});
    relocate(calendar,top,if top {12}else{58});
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
        (start.is_visible() && !start.has_css_class("melania-start-concealed")) ||
        [quick,notifications,calendar].iter().any(|w|
            w.is_visible() && !w.has_css_class("melania-popup-concealed"))
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
            if hover.get() ||
               (sm.is_visible() && !sm.has_css_class("melania-start-concealed")) ||
               [&qs,&nt,&cal].iter().any(|w|
                   w.is_visible() && !w.has_css_class("melania-popup-concealed")) {return;}
            let p=prefs.borrow();
            if p.panel_auto_hide {set_revealed(&w,&p,false);}
        });
    });
    bar.add_controller(motion);
    let panel=panel.clone();let bar=bar.clone();
    let start=start.clone();let quick=quick.clone();
    let notifications=notifications.clone();let calendar=calendar.clone();
    let preferences_file=melania_config::config_path().ok();
    let mut previous_stamp=preferences_file.as_deref().and_then(config_stamp);
    glib::timeout_add_local(Duration::from_millis(500),move ||{
        // A 500ms stat is cheaper than opening, reading and parsing settings
        // twice a second. An atomic rename changes inode, even for same-size
        // values, so normal Settings writes are never silently ignored.
        let current_stamp=preferences_file.as_deref().and_then(config_stamp);
        if current_stamp!=previous_stamp {
            previous_stamp=current_stamp;
            if let Ok(latest)=melania_config::load(){
                if *active.borrow()!=latest {
                    let reveal=!latest.panel_auto_hide || hover.get() ||
                        panel_open(&start,&quick,&notifications,&calendar);
                    apply_preferences(&panel,&bar,&start,&quick,&notifications,&calendar,
                        &latest,&provider,reveal);
                    *active.borrow_mut()=latest;
                }
            }
        }
        if active.borrow().panel_auto_hide {
            // Auto-hide and transient popups still need a cheap periodic
            // reveal check even when preferences remain unchanged.
            let showing=hover.get()||
                panel_open(&start,&quick,&notifications,&calendar);
            set_revealed(&panel,&active.borrow(),showing);
        }
        glib::ControlFlow::Continue
    });
}


#[cfg(test)]
mod aero_tests {
    use super::*;
    #[test]
    fn glass_is_translucent_by_default_and_obeys_explicit_panel_opacity() {
        let mut p=Preferences::default();
        assert_eq!(p.appearance_theme(),"aero");
        assert!(glass_opacity(&p)<0.80);
        p.panel_opacity=60;
        assert!((glass_opacity(&p)-0.60).abs()<0.0001);
    }
    #[test]
    fn glass_strength_controls_alpha_but_does_not_change_light_dark() {
        let mut p=Preferences::default();
        p.blur_strength=0;
        let clear=glass_opacity(&p);
        p.blur_strength=100;
        assert!(glass_opacity(&p)<clear);
        p.set_appearance_theme("light");
        assert!((glass_opacity(&p)-0.94).abs()<0.0001);
        p.set_appearance_theme("dark");
        assert!((glass_opacity(&p)-0.94).abs()<0.0001);
    }
    #[test]
    fn high_contrast_disables_translucent_aero_policy() {
        let mut p=Preferences::default();
        p.high_contrast=true;
        assert!((glass_opacity(&p)-0.94).abs()<0.0001);
    }
}
