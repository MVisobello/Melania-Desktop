//! Physical keyboard policy for COSMIC's Super+X then direction swap.
//! Only normalized evdev keys are considered. No Wayland/host key injection.
#![forbid(unsafe_code)]
use crate::auto_tiling::TileDirection;
const META_L:u32=125;
const META_R:u32=126;
const SHIFT_L:u32=42;
const SHIFT_R:u32=54;
const CTRL_L:u32=29;
const CTRL_R:u32=97;
const ALT_L:u32=56;
const ALT_R:u32=100;
const X:u32=45;
const LEFT:u32=105;
const RIGHT:u32=106;
const UP:u32=103;
const DOWN:u32=108;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SwapKeyAction {
    Forward, Consume, Arm, Swap(TileDirection),
}

#[derive(Default,Debug)]
pub struct TileSwapKeyHandler {
    super_l:bool,super_r:bool,
    shift_l:bool,shift_r:bool,
    ctrl_l:bool,ctrl_r:bool,
    alt_l:bool,alt_r:bool,
    x_held:bool,
    pending:bool,
    arrows_held:u8,
}
impl TileSwapKeyHandler {
    pub fn reset(&mut self){*self=Self::default();}
    fn eligible(&self)->bool {
        (self.super_l || self.super_r) &&
        !(self.shift_l || self.shift_r || self.ctrl_l || self.ctrl_r ||
          self.alt_l || self.alt_r)
    }
    pub fn event(&mut self,code:Option<u32>,pressed:bool)->SwapKeyAction {
        match code {
            Some(META_L)=>{self.super_l=pressed;if !self.super_l && !self.super_r {
                self.pending=false;
            } return SwapKeyAction::Forward;},
            Some(META_R)=>{self.super_r=pressed;if !self.super_l && !self.super_r {
                self.pending=false;
            } return SwapKeyAction::Forward;},
            Some(SHIFT_L)=>{self.shift_l=pressed;if pressed {self.pending=false;}},
            Some(SHIFT_R)=>{self.shift_r=pressed;if pressed {self.pending=false;}},
            Some(CTRL_L)=>{self.ctrl_l=pressed;if pressed {self.pending=false;}},
            Some(CTRL_R)=>{self.ctrl_r=pressed;if pressed {self.pending=false;}},
            Some(ALT_L)=>{self.alt_l=pressed;if pressed {self.pending=false;}},
            Some(ALT_R)=>{self.alt_r=pressed;if pressed {self.pending=false;}},
            Some(X)=>{
                if !pressed {
                    if self.x_held {self.x_held=false;return SwapKeyAction::Consume;}
                } else if self.x_held {
                    return SwapKeyAction::Consume;
                } else if self.eligible() {
                    self.x_held=true;
                    self.pending=true;
                    return SwapKeyAction::Arm;
                }
            },
            Some(LEFT|RIGHT|UP|DOWN)=>{
                let key=code.expect("matched Some");
                let bit=match key {LEFT=>1,RIGHT=>2,UP=>4,DOWN=>8,_=>unreachable!()};
                if !pressed {
                    if self.arrows_held & bit != 0 {
                        self.arrows_held &= !bit;
                        return SwapKeyAction::Consume;
                    }
                } else if self.arrows_held & bit != 0 {
                    return SwapKeyAction::Consume;
                } else if self.pending && self.eligible() {
                    self.pending=false;
                    self.arrows_held |=bit;
                    let direction=match key {
                        LEFT=>TileDirection::Left,RIGHT=>TileDirection::Right,
                        UP=>TileDirection::Up,DOWN=>TileDirection::Down,_=>unreachable!(),
                    };
                    return SwapKeyAction::Swap(direction);
                }
            },
            _=>{}
        }
        SwapKeyAction::Forward
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn super_x_then_direction_swaps_only_once_per_press() {
        let mut h=TileSwapKeyHandler::default();
        h.event(Some(META_L),true);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Arm);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Consume);
        assert_eq!(h.event(Some(X),false),SwapKeyAction::Consume);
        assert_eq!(h.event(Some(RIGHT),true),SwapKeyAction::Swap(TileDirection::Right));
        assert_eq!(h.event(Some(RIGHT),true),SwapKeyAction::Consume);
        assert_eq!(h.event(Some(RIGHT),false),SwapKeyAction::Consume);
        assert_eq!(h.event(Some(DOWN),true),SwapKeyAction::Forward);
        h.event(Some(DOWN),false);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Arm);
        h.event(Some(X),false);
        assert_eq!(h.event(Some(DOWN),true),SwapKeyAction::Swap(TileDirection::Down));
    }
    #[test]
    fn super_release_and_modifier_changes_cancel_pending_swap() {
        let mut h=TileSwapKeyHandler::default();
        h.event(Some(META_R),true);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Arm);
        h.event(Some(META_R),false);
        h.event(Some(X),false);
        assert_eq!(h.event(Some(LEFT),true),SwapKeyAction::Forward);
        h.event(Some(LEFT),false);
        h.event(Some(META_L),true);
        h.event(Some(X),true);
        h.event(Some(X),false);
        h.event(Some(CTRL_L),true);
        h.event(Some(CTRL_L),false);
        assert_eq!(h.event(Some(UP),true),SwapKeyAction::Forward);
        h.reset();
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Forward);
    }
    #[test]
    fn unarmed_super_arrows_and_super_shift_arrows_remain_native_snap_keys() {
        let mut h=TileSwapKeyHandler::default();
        h.event(Some(META_L),true);
        assert_eq!(h.event(Some(LEFT),true),SwapKeyAction::Forward);
        h.event(Some(LEFT),false);
        h.event(Some(SHIFT_L),true);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Forward);
        h.event(Some(X),false);
        assert_eq!(h.event(Some(RIGHT),true),SwapKeyAction::Forward);
        h.event(Some(RIGHT),false);
        h.event(Some(SHIFT_L),false);
        h.event(Some(ALT_L),true);
        assert_eq!(h.event(Some(X),true),SwapKeyAction::Forward);
    }
    #[test]
    fn reset_on_seat_pause_discards_pressed_key_serials() {
        let mut h=TileSwapKeyHandler::default();
        h.event(Some(META_L),true);
        h.event(Some(X),true);
        h.reset();
        assert_eq!(h.event(Some(RIGHT),false),SwapKeyAction::Forward);
        assert_eq!(h.event(Some(RIGHT),true),SwapKeyAction::Forward);
        assert_eq!(h.event(Some(X),false),SwapKeyAction::Forward);
    }
}
