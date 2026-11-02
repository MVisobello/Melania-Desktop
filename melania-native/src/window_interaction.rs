//! QEMU-only bounded pointer interaction model for two Wayland windows.
//!
//! Movement and resize are driven exclusively by a real left-button press
//! while a libseat/libinput session is active. Geometry remains separate from
//! the framebuffer submission: moving/resizing does not claim a KMS repaint.
const OUT_W:i32=1280;
const OUT_H:i32=800;
const MIN_W:i32=96;
const MIN_H:i32=72;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowRect {
    pub x:i32,
    pub y:i32,
    pub width:i32,
    pub height:i32,
}

impl WindowRect {
    pub fn valid(self)->bool {
        self.x>=0 && self.y>=0 &&
        self.width>=MIN_W && self.height>=MIN_H &&
        self.x.checked_add(self.width).is_some_and(|r|r<=OUT_W) &&
        self.y.checked_add(self.height).is_some_and(|b|b<=OUT_H)
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum InteractionKind { Move, ResizeBottomRight }

#[derive(Clone,Copy,Debug,PartialEq)]
struct Active {
    key:u64,
    kind:InteractionKind,
    origin:WindowRect,
    x0:f64,
    y0:f64,
}

#[derive(Debug,Default)]
pub struct WindowInteraction {
    active:Option<Active>,
}

impl WindowInteraction {
    pub fn active_key(&self)->Option<u64> { self.active.map(|a|a.key) }
    pub fn active_kind(&self)->Option<InteractionKind> { self.active.map(|a|a.kind) }

    /// Caller must validate a real compositor-owned left-button press.
    /// Never transfer an active grab to another client without a release.
    pub fn begin(&mut self,key:u64,rect:WindowRect,x:f64,y:f64,
                 kind:InteractionKind)->bool {
        if key==0 || self.active.is_some() || !rect.valid() ||
           !x.is_finite() || !y.is_finite() ||
           !(0.0..OUT_W as f64).contains(&x) ||
           !(0.0..OUT_H as f64).contains(&y)
        {
            return false;
        }
        self.active=Some(Active{key,kind,origin:rect,x0:x,y0:y});
        true
    }

    /// Pointer motion is bounded to the current virtual output and respects
    /// the minimum client size. Return geometry without changing the scene:
    /// the caller chooses when an actual Wayland scene is ready to update.
    pub fn motion(&self,x:f64,y:f64)->Option<(u64,WindowRect)> {
        let active=self.active?;
        if !x.is_finite() || !y.is_finite() {return None;}
        let x=x.clamp(0.0,(OUT_W-1) as f64);
        let y=y.clamp(0.0,(OUT_H-1) as f64);
        let dx=(x-active.x0).round() as i32;
        let dy=(y-active.y0).round() as i32;
        let mut rect=active.origin;
        match active.kind {
            InteractionKind::Move=>{
                rect.x=(rect.x.saturating_add(dx)).clamp(0,OUT_W-rect.width);
                rect.y=(rect.y.saturating_add(dy)).clamp(0,OUT_H-rect.height);
            },
            InteractionKind::ResizeBottomRight=>{
                rect.width=(rect.width.saturating_add(dx))
                    .clamp(MIN_W,OUT_W-rect.x);
                rect.height=(rect.height.saturating_add(dy))
                    .clamp(MIN_H,OUT_H-rect.y);
            },
        }
        Some((active.key,rect))
    }

    /// A real left-button release, libseat pause, disconnect or teardown
    /// revokes the current grab. A late motion is then ignored.
    pub fn end(&mut self)->Option<u64> { self.active.take().map(|a|a.key) }
    pub fn cancel(&mut self) { self.active=None; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect()->WindowRect {WindowRect{x:160,y:120,width:360,height:224}}

    #[test]
    fn drag_moves_without_resizing_and_bounds_to_virtual_output() {
        let mut model=WindowInteraction::default();
        assert!(model.begin(1,rect(),200.0,130.0,InteractionKind::Move));
        assert!(!model.begin(2,rect(),800.0,200.0,InteractionKind::Move));
        assert_eq!(model.active_key(),Some(1));
        assert_eq!(model.motion(350.0,300.0),
            Some((1,WindowRect{x:310,y:290,..rect()})));
        assert_eq!(model.motion(1279.0,799.0),
            Some((1,WindowRect{x:920,y:576,..rect()})));
        assert_eq!(model.end(),Some(1));
        assert_eq!(model.motion(300.0,200.0),None);
    }

    #[test]
    fn resize_bottom_right_obeys_minimum_and_display_bounds() {
        let mut model=WindowInteraction::default();
        assert!(model.begin(2,WindowRect{x:650,y:190,..rect()},
                            1000.0,410.0,InteractionKind::ResizeBottomRight));
        assert_eq!(model.motion(1279.0,799.0),
            Some((2,WindowRect{x:650,y:190,width:630,height:610})));
        assert_eq!(model.motion(0.0,0.0),
            Some((2,WindowRect{x:650,y:190,width:96,height:72})));
        model.cancel();
        assert_eq!(model.active_key(),None);
    }

    #[test]
    fn invalid_client_id_bad_coordinates_and_geometry_are_rejected() {
        let mut model=WindowInteraction::default();
        assert!(!model.begin(0,rect(),200.0,140.0,InteractionKind::Move));
        assert!(!model.begin(1,rect(),f64::NAN,140.0,InteractionKind::Move));
        assert!(!model.begin(1,rect(),f64::INFINITY,140.0,InteractionKind::Move));
        assert!(!model.begin(1,rect(),2000.0,140.0,InteractionKind::Move));
        assert!(!model.begin(1,WindowRect{x:-1,..rect()},140.0,140.0,
                            InteractionKind::Move));
        assert!(!model.begin(1,WindowRect{width:50,..rect()},140.0,140.0,
                            InteractionKind::Move));
        assert!(!model.begin(1,WindowRect{x:i32::MAX,..rect()},140.0,140.0,
                            InteractionKind::Move));
        assert!(model.begin(1,rect(),140.0,140.0,InteractionKind::Move));
        assert_eq!(model.motion(f64::NAN,140.0),None);
        model.cancel();
        assert_eq!(model.motion(140.0,140.0),None);
    }
}
