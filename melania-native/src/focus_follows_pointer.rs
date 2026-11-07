//! QEMU-native COSMIC-style focus-follows-pointer safety policy.
//! Real libinput/Wayland wiring lives in wayland_bridge; this pure Rust gate
//! makes dangerous conditions explicit and testable before keyboard focus.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]
pub struct HoverFocusGate {
    pub enabled:bool,
    pub seat_active:bool,
    pub revoked:bool,
    pub button_held:bool,
    pub interactive_grab:bool,
    pub popup_grab:bool,
    pub visible_toplevel:bool,
    pub already_focused:bool,
}
pub fn should_focus_on_motion(gate:HoverFocusGate)->bool {
    gate.enabled && gate.seat_active && !gate.revoked &&
        !gate.button_held && !gate.interactive_grab && !gate.popup_grab &&
        gate.visible_toplevel && !gate.already_focused
}
#[cfg(test)]
mod tests {
    use super::*;
    fn eligible()->HoverFocusGate {
        HoverFocusGate {enabled:true,seat_active:true,
            visible_toplevel:true,..Default::default()}
    }
    #[test]
    fn hover_focus_requires_explicit_opt_in() {
        let mut g=eligible();g.enabled=false;
        assert!(!should_focus_on_motion(g));
        assert!(should_focus_on_motion(eligible()));
    }
    #[test]
    fn never_focus_while_input_or_seat_ownership_is_unsafe() {
        let baseline=eligible();
        for g in [
            HoverFocusGate{seat_active:false,..baseline},
            HoverFocusGate{revoked:true,..baseline},
            HoverFocusGate{button_held:true,..baseline},
            HoverFocusGate{interactive_grab:true,..baseline},
            HoverFocusGate{popup_grab:true,..baseline},
        ] {
            assert!(!should_focus_on_motion(g),"{g:?}");
        }
    }
    #[test]
    fn background_popups_and_current_window_do_not_transfer_focus() {
        let g=eligible();
        assert!(!should_focus_on_motion(HoverFocusGate{
            visible_toplevel:false,..g
        }));
        assert!(!should_focus_on_motion(HoverFocusGate{
            already_focused:true,..g
        }));
    }
}
