//! Settings icons are sourced consistently from the active freedesktop theme.
//! Only the official Melania crescent is drawn separately from its icon theme.
use gtk::prelude::*;
use gtk::{cairo, DrawingArea, Image, Widget};

pub fn icon(name: &str, pixels: i32) -> Widget {
    if name == "melania" {
        // Official blue open crescent with its inner blue dot, not the
        // generated pink picture placeholder returned by some icon themes.
        let area = DrawingArea::new();
        area.set_content_width(pixels);
        area.set_content_height(pixels);
        area.set_size_request(pixels, pixels);
        area.set_draw_func(move |_, ctx, width, height| {
            ctx.scale(width as f64 / 24.0, height as f64 / 24.0);
            ctx.new_path();
            ctx.arc(10.2, 12.0, 9.0, 0.0, std::f64::consts::TAU);
            ctx.arc(14.4, 9.6, 7.0, 0.0, std::f64::consts::TAU);
            ctx.set_fill_rule(cairo::FillRule::EvenOdd);
            let gradient = cairo::LinearGradient::new(1.0, 2.0, 23.0, 24.0);
            gradient.add_color_stop_rgb(0.0, 0.34, 0.77, 1.0);
            gradient.add_color_stop_rgb(1.0, 0.02, 0.37, 0.80);
            let _ = ctx.set_source(&gradient);
            let _ = ctx.fill();
            ctx.set_fill_rule(cairo::FillRule::Winding);
            ctx.arc(15.7, 13.3, 2.1, 0.0, std::f64::consts::TAU);
            ctx.set_source_rgb(0.09, 0.55, 0.94);
            let _ = ctx.fill();
        });
        return area.upcast();
    }
    // All Settings glyphs resolve from the active Melania Fluent icon theme.
    // Mixing selected legacy illustrations with separate Cairo tiles produced
    // visibly different icon families in the same GNOME-style navigation.
    let image = Image::from_icon_name(name);
    image.set_pixel_size(pixels);
    image.add_css_class("settings-semantic-icon");
    image.upcast()
}
