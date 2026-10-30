//! Pure Rust, compositor-independent pointer interaction geometry.
//!
//! All operations start from an immutable rectangle and the press position.
//! This prevents accumulated rounding drift when the compositor receives
//! hundreds of pointer motion events. No GTK/Smithay dependency is required.
use crate::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edges(u8);
impl Edges {
    pub const TOP: Self = Self(1);
    pub const BOTTOM: Self = Self(2);
    pub const LEFT: Self = Self(4);
    pub const RIGHT: Self = Self(8);
    pub const TOP_LEFT: Self = Self(5);
    pub const BOTTOM_LEFT: Self = Self(6);
    pub const TOP_RIGHT: Self = Self(9);
    pub const BOTTOM_RIGHT: Self = Self(10);
    pub fn from_xdg(raw: u32) -> Option<Self> {
        match raw { 1 | 2 | 4 | 5 | 6 | 8 | 9 | 10 => Some(Self(raw as u8)), _ => None }
    }
    pub fn has(self, edge: Self) -> bool { self.0 & edge.0 != 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Move, Resize(Edges) }

#[derive(Clone, Copy, Debug)]
pub struct Interaction {
    pub original: Rect,
    pub pointer_start: (f64, f64),
    pub operation: Operation,
}
impl Interaction {
    pub fn new(original: Rect, pointer_start: (f64, f64), operation: Operation) -> Self {
        Self { original, pointer_start, operation }
    }
    /// Compute the new frame at the current pointer location.
    /// Keep a sensible minimum, and saturate enormous or malformed movement.
    pub fn updated(&self, pointer: (f64, f64)) -> Rect {
        fn delta(a: f64, b: f64) -> i64 {
            if !a.is_finite() || !b.is_finite() { return 0; }
            (a - b).round().clamp(-100_000.0, 100_000.0) as i64
        }
        fn integer(v: i64) -> i32 { v.clamp(i32::MIN as i64, i32::MAX as i64) as i32 }
        let dx = delta(pointer.0, self.pointer_start.0);
        let dy = delta(pointer.1, self.pointer_start.1);
        let mut l = self.original.x as i64;
        let mut t = self.original.y as i64;
        let mut r = l + self.original.width as i64;
        let mut b = t + self.original.height as i64;
        match self.operation {
            Operation::Move => { l += dx; r += dx; t += dy; b += dy; }
            Operation::Resize(edges) => {
                if edges.has(Edges::LEFT) { l = (l + dx).min(r - 120); }
                if edges.has(Edges::RIGHT) { r = (r + dx).max(l + 120); }
                if edges.has(Edges::TOP) { t = (t + dy).min(b - 80); }
                if edges.has(Edges::BOTTOM) { b = (b + dy).max(t + 80); }
            }
        }
        Rect { x: integer(l), y: integer(t), width: (r - l).clamp(1, u32::MAX as i64) as u32,
            height: (b - t).clamp(1, u32::MAX as i64) as u32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn initial() -> Rect { Rect { x: 100, y: 80, width: 640, height: 480 } }
    #[test]
    fn drag_moves_from_original_without_drift() {
        let drag = Interaction::new(initial(), (7.0, 12.0), Operation::Move);
        assert_eq!(drag.updated((37.0, 50.0)), Rect {x:130,y:118,width:640,height:480});
        assert_eq!(drag.updated((7.0, 12.0)), initial());
    }
    #[test]
    fn resize_right_bottom_changes_size_only() {
        let drag = Interaction::new(initial(), (10.0, 10.0), Operation::Resize(Edges::BOTTOM_RIGHT));
        assert_eq!(drag.updated((30.0, 35.0)),Rect{x:100,y:80,width:660,height:505});
    }
    #[test]
    fn resize_top_left_preserves_opposite_corner() {
        let drag = Interaction::new(initial(), (10.0, 10.0), Operation::Resize(Edges::TOP_LEFT));
        assert_eq!(drag.updated((30.0, 35.0)),Rect{x:120,y:105,width:620,height:455});
    }
    #[test]
    fn extreme_resize_cannot_invert_or_collapse_window() {
        let drag = Interaction::new(initial(), (0.0, 0.0), Operation::Resize(Edges::TOP_LEFT));
        assert_eq!(drag.updated((9999.0, 9999.0)), Rect{x:620,y:480,width:120,height:80});
    }
    #[test]
    fn unsupported_edge_rejected_and_nan_ignored() {
        assert_eq!(Edges::from_xdg(3), None);
        assert_eq!(Edges::from_xdg(9),Some(Edges::TOP_RIGHT));
        let drag = Interaction::new(initial(), (0.0, 0.0), Operation::Move);
        assert_eq!(drag.updated((f64::NAN, f64::INFINITY)),initial());
    }
}
