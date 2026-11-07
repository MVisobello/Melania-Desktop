//! Trusted libinput Super+S stack actions for a disposable native QEMU seat.
//! Evdev codes are normalized from Smithay XKB keycode minus 8.
//! This module sends no input, client configure or DRM operations.
#![forbid(unsafe_code)]

const LEFT_META:u32=125;
const RIGHT_META:u32=126;
const LEFT_SHIFT:u32=42;
const RIGHT_SHIFT:u32=54;
const S:u32=31;
const PAGE_UP:u32=104;
const PAGE_DOWN:u32=109;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TabKeyAction {Forward,Consume,ToggleStack,Attach,Next,Previous}

#[derive(Default,Debug)]
pub struct TabKeyHandler {
    meta_l:bool,meta_r:bool,
    shift_l:bool,shift_r:bool,
    held:u8,
}
impl TabKeyHandler {
    pub fn reset(&mut self){*self=Self::default();}
    pub fn event(&mut self,code:Option<u32>,pressed:bool)->TabKeyAction {
        match code {
            Some(LEFT_META)=>{
                self.meta_l=pressed;
                return TabKeyAction::Forward;
            },
            Some(RIGHT_META)=>{
                self.meta_r=pressed;
                return TabKeyAction::Forward;
            },
            Some(LEFT_SHIFT)=>{
                self.shift_l=pressed;
                return TabKeyAction::Forward;
            },
            Some(RIGHT_SHIFT)=>{
                self.shift_r=pressed;
                return TabKeyAction::Forward;
            },
            Some(S|PAGE_UP|PAGE_DOWN)=>{
                let key=code.expect("matched Some");
                let mask=match key {S=>1,PAGE_UP=>2,PAGE_DOWN=>4,_=>unreachable!()};
                if !pressed {
                    if self.held & mask!=0 {
                        self.held &= !mask;
                        return TabKeyAction::Consume;
                    }
                    return TabKeyAction::Forward;
                }
                if self.held & mask!=0 {return TabKeyAction::Consume;}
                if !(self.meta_l || self.meta_r) {return TabKeyAction::Forward;}
                self.held|=mask;
                match key {
                    S=>if self.shift_l || self.shift_r {
                        TabKeyAction::Attach
                    } else {TabKeyAction::ToggleStack},
                    PAGE_UP=>TabKeyAction::Previous,
                    PAGE_DOWN=>TabKeyAction::Next,
                    _=>unreachable!(),
                }
            },
            _=>TabKeyAction::Forward,
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn super_s_stacking_and_shift_s_attach_require_real_modifier(){
        let mut h=TabKeyHandler::default();
        assert_eq!(h.event(Some(S),true),TabKeyAction::Forward);
        h.event(Some(S),false);
        h.event(Some(LEFT_META),true);
        assert_eq!(h.event(Some(S),true),TabKeyAction::ToggleStack);
        assert_eq!(h.event(Some(S),true),TabKeyAction::Consume);
        assert_eq!(h.event(Some(S),false),TabKeyAction::Consume);
        h.event(Some(LEFT_SHIFT),true);
        assert_eq!(h.event(Some(S),true),TabKeyAction::Attach);
        assert_eq!(h.event(Some(S),false),TabKeyAction::Consume);
        h.event(Some(LEFT_SHIFT),false);
        h.event(Some(LEFT_META),false);
        assert_eq!(h.event(Some(S),true),TabKeyAction::Forward);
    }
    #[test]
    fn super_page_keys_cycle_without_sticky_auto_repeat(){
        let mut h=TabKeyHandler::default();
        h.event(Some(RIGHT_META),true);
        assert_eq!(h.event(Some(PAGE_DOWN),true),TabKeyAction::Next);
        assert_eq!(h.event(Some(PAGE_DOWN),true),TabKeyAction::Consume);
        assert_eq!(h.event(Some(PAGE_DOWN),false),TabKeyAction::Consume);
        assert_eq!(h.event(Some(PAGE_UP),true),TabKeyAction::Previous);
        assert_eq!(h.event(Some(PAGE_UP),false),TabKeyAction::Consume);
        h.reset();
        assert_eq!(h.event(Some(PAGE_DOWN),false),TabKeyAction::Forward);
        assert_eq!(h.event(Some(PAGE_DOWN),true),TabKeyAction::Forward);
    }
}
