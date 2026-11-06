//! Input policy for Super+arrows. Only trusted libinput evdev codes are accepted.
//! This module never sends a Wayland configure or a synthetic key event.
use crate::snap_layout::SnapZone;
const LEFT_META:u32=125;
const RIGHT_META:u32=126;
const UP:u32=103;
const LEFT:u32=105;
const RIGHT:u32=106;
const DOWN:u32=108;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SnapKeyAction {
    Forward, Consume, Snap(SnapZone), Maximize, Restore,
}

#[derive(Default,Debug)]
pub struct SnapKeyHandler {
    left_meta:bool,
    right_meta:bool,
    arrows_held:u8,
    horizontal:Option<SnapZone>,
}

impl SnapKeyHandler {
    pub fn reset(&mut self){*self=Self::default();}
    pub fn event(&mut self,code:Option<u32>,pressed:bool)->SnapKeyAction {
        match code {
            Some(LEFT_META)=>{
                self.left_meta=pressed;
                if !pressed && !self.right_meta {self.horizontal=None;}
                SnapKeyAction::Forward
            },
            Some(RIGHT_META)=>{
                self.right_meta=pressed;
                if !pressed && !self.left_meta {self.horizontal=None;}
                SnapKeyAction::Forward
            },
            Some(LEFT|RIGHT|UP|DOWN)=>{
                let code=code.expect("matched Some");
                let flag=match code {
                    LEFT=>1,RIGHT=>2,UP=>4,DOWN=>8,_=>unreachable!()
                };
                if !pressed {
                    if self.arrows_held & flag != 0 {
                        self.arrows_held &= !flag;
                        return SnapKeyAction::Consume;
                    }
                    return SnapKeyAction::Forward;
                }
                if !(self.left_meta || self.right_meta) {
                    return SnapKeyAction::Forward;
                }
                // Ignore auto-repeat: each distinct physical press only
                // changes geometry once, even if the client is slow to ACK.
                if self.arrows_held & flag != 0 {
                    return SnapKeyAction::Consume;
                }
                self.arrows_held |= flag;
                match code {
                    LEFT=>{self.horizontal=Some(SnapZone::LeftHalf); SnapKeyAction::Snap(SnapZone::LeftHalf)},
                    RIGHT=>{self.horizontal=Some(SnapZone::RightHalf); SnapKeyAction::Snap(SnapZone::RightHalf)},
                    UP=>match self.horizontal.take() {
                        Some(SnapZone::LeftHalf)=>SnapKeyAction::Snap(SnapZone::TopLeft),
                        Some(SnapZone::RightHalf)=>SnapKeyAction::Snap(SnapZone::TopRight),
                        _=>SnapKeyAction::Maximize,
                    },
                    DOWN=>match self.horizontal.take() {
                        Some(SnapZone::LeftHalf)=>SnapKeyAction::Snap(SnapZone::BottomLeft),
                        Some(SnapZone::RightHalf)=>SnapKeyAction::Snap(SnapZone::BottomRight),
                        _=>SnapKeyAction::Restore,
                    },
                    _=>unreachable!(),
                }
            },
            _=>SnapKeyAction::Forward,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_arrows_pass_through_and_super_arrows_are_intercepted() {
        let mut s=SnapKeyHandler::default();
        assert_eq!(s.event(Some(LEFT),true),SnapKeyAction::Forward);
        assert_eq!(s.event(Some(LEFT),false),SnapKeyAction::Forward);
        assert_eq!(s.event(Some(LEFT_META),true),SnapKeyAction::Forward);
        assert_eq!(s.event(Some(LEFT),true),SnapKeyAction::Snap(SnapZone::LeftHalf));
        assert_eq!(s.event(Some(LEFT),true),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(LEFT),false),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(RIGHT),true),SnapKeyAction::Snap(SnapZone::RightHalf));
        assert_eq!(s.event(Some(RIGHT),false),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(UP),true),SnapKeyAction::Snap(SnapZone::TopRight));
        assert_eq!(s.event(Some(UP),false),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(DOWN),true),SnapKeyAction::Restore);
        assert_eq!(s.event(Some(DOWN),false),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(LEFT_META),false),SnapKeyAction::Forward);
    }
    #[test]
    fn super_horizontal_then_vertical_snaps_to_quadrants() {
        for (horizontal,upper,lower) in [
            (LEFT,SnapZone::TopLeft,SnapZone::BottomLeft),
            (RIGHT,SnapZone::TopRight,SnapZone::BottomRight),
        ] {
            let mut s=SnapKeyHandler::default();
            s.event(Some(LEFT_META),true);
            assert!(matches!(s.event(Some(horizontal),true),SnapKeyAction::Snap(_)));
            assert_eq!(s.event(Some(horizontal),false),SnapKeyAction::Consume);
            assert_eq!(s.event(Some(UP),true),SnapKeyAction::Snap(upper));
            assert_eq!(s.event(Some(UP),false),SnapKeyAction::Consume);
            assert_eq!(s.event(Some(UP),true),SnapKeyAction::Maximize);
            assert_eq!(s.event(Some(UP),false),SnapKeyAction::Consume);
            assert!(matches!(s.event(Some(horizontal),true),SnapKeyAction::Snap(_)));
            assert_eq!(s.event(Some(horizontal),false),SnapKeyAction::Consume);
            assert_eq!(s.event(Some(DOWN),true),SnapKeyAction::Snap(lower));
            assert_eq!(s.event(Some(DOWN),false),SnapKeyAction::Consume);
            assert_eq!(s.event(Some(DOWN),true),SnapKeyAction::Restore);
            assert_eq!(s.event(Some(DOWN),false),SnapKeyAction::Consume);
            s.event(Some(LEFT_META),false);
        }
    }
    #[test]
    fn releasing_last_super_key_discards_pending_quadrant() {
        let mut s=SnapKeyHandler::default();
        s.event(Some(LEFT_META),true);
        s.event(Some(LEFT),true);
        s.event(Some(LEFT),false);
        s.event(Some(LEFT_META),false);
        s.event(Some(LEFT_META),true);
        assert_eq!(s.event(Some(UP),true),SnapKeyAction::Maximize);
        s.event(Some(UP),false);
        s.event(Some(LEFT_META),false);
    }
    #[test]
    fn right_super_and_release_order_never_leak_or_repeat_arrows() {
        let mut s=SnapKeyHandler::default();
        s.event(Some(RIGHT_META),true);
        assert_eq!(s.event(Some(LEFT),true),SnapKeyAction::Snap(SnapZone::LeftHalf));
        s.event(Some(RIGHT_META),false);
        assert_eq!(s.event(Some(LEFT),false),SnapKeyAction::Consume);
        assert_eq!(s.event(Some(LEFT),true),SnapKeyAction::Forward);
        assert_eq!(s.event(Some(LEFT),false),SnapKeyAction::Forward);
        s.event(Some(LEFT_META),true);
        s.event(Some(RIGHT_META),true);
        s.event(Some(LEFT_META),false);
        assert_eq!(s.event(Some(UP),true),SnapKeyAction::Maximize);
        s.reset();
        assert_eq!(s.event(Some(UP),false),SnapKeyAction::Forward);
    }
}
