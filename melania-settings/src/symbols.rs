//! Compact semantic icons for Settings (not part of the user-approved Fluent pack).
//! The existing icon theme stays untouched; only Settings' missing sidebar
//! symbols get consistent, original Cairo-rendered 24px pictograms.
use gtk::prelude::*;
use gtk::{cairo, DrawingArea, Image, Widget};

pub fn icon(name: &str, pixels: i32) -> Widget {
    // Prefer the recognizable approved Fluent artwork where already available.
    if matches!(name, "computer" | "network-workgroup" | "video-display" |
                      "drive-harddisk" | "user-desktop" | "folder" | "user-home") {
        let image = Image::from_icon_name(name);
        image.set_pixel_size(pixels);
        return image.upcast();
    }
    let symbol = name.to_owned();
    let area = DrawingArea::new();
    area.set_content_width(pixels);
    area.set_content_height(pixels);
    area.set_size_request(pixels, pixels);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("settings-semantic-icon");
    area.set_draw_func(move |_, ctx, width, height| {
        ctx.scale(width as f64 / 24.0, height as f64 / 24.0);
        paint(ctx, symbol.as_str());
    });
    area.upcast()
}

fn line(ctx: &cairo::Context, coords: &[(f64, f64)]) {
    if let Some((x, y)) = coords.first() {
        ctx.move_to(*x, *y);
        for (x, y) in &coords[1..] { ctx.line_to(*x, *y); }
        let _ = ctx.stroke();
    }
}
fn circle(ctx: &cairo::Context, x: f64, y: f64, r: f64) {
    ctx.arc(x, y, r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
}
fn rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.rectangle(x, y, w, h);
    let _ = ctx.stroke();
}
fn paint(ctx: &cairo::Context, name: &str) {
    let kind = match name {
        "bluetooth" | "bluetooth-active" => "bluetooth",
        "preferences-desktop-theme" | "preferences-desktop-wallpaper" => "palette",
        "preferences-system-notifications" => "bell",
        "applications-other" | "application-x-executable" => "apps",
        "avatar-default" => "account",
        "x-office-calendar" => "calendar",
        "battery" | "battery-full" => "battery",
        "audio-volume-high" | "audio-card" => "audio",
        "security-high" => "shield",
        "preferences-desktop-locale" => "globe",
        "preferences-desktop-accessibility" => "accessibility",
        "dialog-information" => "info",
        "view-grid" | "view-grid-symbolic" => "grid",
        "input-keyboard" => "keyboard",
        "network-wireless" => "wifi",
        _ => "settings",
    };
    let (r, g, b) = match kind {
        "palette" => (0.53, 0.37, 0.85), "bell" => (0.89, 0.55, 0.19),
        "apps" => (0.26, 0.53, 0.79), "account" => (0.29, 0.59, 0.80),
        "calendar" => (0.85, 0.39, 0.33), "battery" => (0.18, 0.61, 0.42),
        "audio" => (0.27, 0.47, 0.75), "shield" => (0.25, 0.57, 0.80),
        "globe" => (0.13, 0.59, 0.74), "accessibility" => (0.25, 0.53, 0.85),
        "info" => (0.25, 0.53, 0.78), "bluetooth" => (0.30, 0.52, 0.81),
        "wifi" => (0.25, 0.52, 0.80), _ => (0.31, 0.49, 0.69),
    };
    // A subtly graduated fluent-like tile rather than a generated letter badge.
    ctx.new_path();
    ctx.arc(5.5, 5.5, 4.0, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    ctx.arc(18.5, 5.5, 4.0, 1.5 * std::f64::consts::PI, 2.0 * std::f64::consts::PI);
    ctx.arc(18.5, 18.5, 4.0, 0.0, 0.5 * std::f64::consts::PI);
    ctx.arc(5.5, 18.5, 4.0, 0.5 * std::f64::consts::PI, std::f64::consts::PI);
    ctx.close_path();
    let gradient = cairo::LinearGradient::new(0.0, 0.0, 0.0, 24.0);
    gradient.add_color_stop_rgb(0.0, (r + 0.13f64).min(1.0), (g + 0.13f64).min(1.0), (b + 0.13f64).min(1.0));
    gradient.add_color_stop_rgb(1.0, r, g, b);
    let _ = ctx.set_source(&gradient);
    let _ = ctx.fill();
    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.set_line_width(1.65);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    match kind {
        "bluetooth" => {
            line(ctx, &[(12.,4.5),(12.,19.5),(16.5,15.),(7.5,8.)]);
            line(ctx, &[(12.,4.5),(16.5,9.),(7.5,16.)]);
        },
        "palette" => {
            circle(ctx, 12., 12., 7.7);
            for (x, y) in [(9.,8.),(14.5,8.5),(16.5,13.5),(8.,15.)] {
                ctx.arc(x, y, 1.0, 0.0, std::f64::consts::TAU);
                let _ = ctx.fill();
            }
        },
        "bell" => {
            line(ctx,&[(6.,16.7),(8.,14.),(8.,10.),(9.,7.5),(11.,6.5),(13.,6.5),(15.,7.5),(16.,10.),(16.,14.),(18.,16.7),(6.,16.7)]);
            line(ctx,&[(10.5,19.),(13.5,19.)]);
        },
        "apps" | "grid" => for x in [6.,13.] { for y in [6.,13.] { rect(ctx,x,y,5.,5.); } },
        "account" => {
            circle(ctx,12.,9.,3.2);
            ctx.arc(12.,19.,6.,std::f64::consts::PI,2.*std::f64::consts::PI);
            let _ = ctx.stroke();
        },
        "calendar" => {
            rect(ctx,5.5,7.,13.,12.);
            line(ctx,&[(5.5,11.),(18.5,11.)]);
            line(ctx,&[(9.,5.),(9.,9.)]);
            line(ctx,&[(15.,5.),(15.,9.)]);
            line(ctx,&[(9.,14.),(11.,14.)]);
        },
        "battery" => {
            rect(ctx,5.,8.,13.8,8.);
            line(ctx,&[(19.,10.),(20.,10.),(20.,14.),(19.,14.)]);
            ctx.rectangle(7.4,10.4,8.,3.2); let _=ctx.fill();
        },
        "audio" => {
            line(ctx,&[(5.5,10.),(9.,10.),(12.8,7.),(12.8,17.),(9.,14.),(5.5,14.),(5.5,10.)]);
            ctx.arc(12.,12.,5.2,-0.9,0.9); let _=ctx.stroke();
            ctx.arc(12.,12.,8.,-0.85,0.85); let _=ctx.stroke();
        },
        "shield" => {
            line(ctx,&[(12.,4.7),(19.,7.2),(18.,13.),(15.,18.),(12.,20.),(9.,18.),(6.,13.),(5.,7.2),(12.,4.7)]);
            line(ctx,&[(8.5,12.5),(11.,15.),(15.5,10.)]);
        },
        "globe" => {
            circle(ctx,12.,12.,7.2);
            line(ctx,&[(4.9,12.),(19.1,12.)]);
            ctx.save().ok();
            ctx.translate(12.,12.);
            ctx.scale(0.42,1.);
            circle(ctx,0.,0.,7.2);
            ctx.restore().ok();
        },
        "accessibility" => {
            circle(ctx,12.,6.8,1.7);
            line(ctx,&[(5.6,11.),(18.4,11.)]);
            line(ctx,&[(12.,11.),(12.,15.5),(8.,19.)]);
            line(ctx,&[(12.,15.5),(16.,19.)]);
        },
        "info" => {
            circle(ctx,12.,12.,7.5);
            line(ctx,&[(12.,10.),(12.,16.3)]);
            ctx.arc(12.,7.5,0.6,0.,std::f64::consts::TAU); let _=ctx.fill();
        },
        "keyboard" => {
            rect(ctx,4.5,7.5,15.,9.);
            for y in [10.,12.5] { for x in [7.,10.,13.,16.] {
                ctx.arc(x,y,0.42,0.,std::f64::consts::TAU); let _=ctx.fill();
            }}
            line(ctx,&[(8.,15.),(16.,15.)]);
        },
        "wifi" => {
            ctx.arc(12.,16.,10.,-2.4,-0.74); let _=ctx.stroke();
            ctx.arc(12.,16.,6.5,-2.4,-0.74); let _=ctx.stroke();
            ctx.arc(12.,16.,3.,-2.4,-0.74); let _=ctx.stroke();
            circle(ctx,12.,16.,0.4);
        },
        _ => {
            circle(ctx,12.,12.,5.);
            circle(ctx,12.,12.,2.);
            line(ctx,&[(12.,4.5),(12.,6.5)]);
            line(ctx,&[(12.,17.5),(12.,19.5)]);
        }
    }
}
