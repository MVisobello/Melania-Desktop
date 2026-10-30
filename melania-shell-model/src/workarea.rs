//! Pure Wayland work-area placement rules; no fixed taskbar-height guesses.
//! The compositor supplies the live non-exclusive zone reported by Smithay.
use super::Geometry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitialPlacement {
    pub x: i32,
    pub y: i32,
    pub requested_width: u32,
    pub requested_height: u32,
    /// Request a client resize only if the initial buffer won't fit.
    pub needs_resize: bool,
}
/// Place a newly mapped normal window wholly inside the usable desktop
/// whenever the client complies with requested dimensions. Preserve the
/// cascade origin whenever it already fits. Never shift the panel.
pub fn fit_initial_window(usable: Geometry, window: Geometry) -> Option<InitialPlacement> {
    if usable.width == 0 || usable.height == 0 || window.width == 0 || window.height == 0 {
        return None;
    }
    let w = window.width.min(usable.width);
    let h = window.height.min(usable.height);
    let max_x = usable.right().saturating_sub(i64::from(w));
    let max_y = usable.bottom().saturating_sub(i64::from(h));
    let x = (i64::from(window.x).max(i64::from(usable.x)).min(max_x))
        .clamp(i64::from(i32::MIN),i64::from(i32::MAX)) as i32;
    let y = (i64::from(window.y).max(i64::from(usable.y)).min(max_y))
        .clamp(i64::from(i32::MIN),i64::from(i32::MAX)) as i32;
    Some(InitialPlacement {
        x, y, requested_width: w, requested_height: h,
        needs_resize: window.width > usable.width || window.height > usable.height,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn area() -> Geometry {Geometry::new(0,0,1280,744)}
    #[test] fn respects_bottom_panel_and_cascade() {
        let placed=fit_initial_window(area(),Geometry::new(260,180,1160,655)).unwrap();
        assert_eq!((placed.x,placed.y),(120,89));
        assert!(!placed.needs_resize);
    }
    #[test] fn untouched_when_already_visible() {
        let placed=fit_initial_window(area(),Geometry::new(45,35,700,550)).unwrap();
        assert_eq!((placed.x,placed.y),(45,35));
        assert!(!placed.needs_resize);
    }
    #[test] fn oversized_clients_get_a_bounded_size_request() {
        let placed=fit_initial_window(area(),Geometry::new(200,200,1900,1000)).unwrap();
        assert_eq!((placed.x,placed.y),(0,0));
        assert_eq!((placed.requested_width,placed.requested_height),(1280,744));
        assert!(placed.needs_resize);
    }
    #[test] fn offset_output_and_top_panel() {
        let placed=fit_initial_window(Geometry::new(1280,36,1024,680),
            Geometry::new(1200,-10,500,400)).unwrap();
        assert_eq!((placed.x,placed.y),(1280,36));
    }
    #[test] fn invalid_or_zero_sized_geometry_is_deferred() {
        assert_eq!(fit_initial_window(Geometry::new(0,0,0,640),Geometry::new(20,20,400,300)),None);
        assert_eq!(fit_initial_window(area(),Geometry::new(0,0,0,300)),None);
    }
    #[test] fn near_i32_extremes_never_overflow() {
        let result=fit_initial_window(Geometry::new(i32::MAX-100,10,500,100),
            Geometry::new(i32::MAX-10,100,300,100)).unwrap();
        assert_eq!(result.requested_width,300);
        assert!(result.x>=i32::MAX-100);
    }
}
