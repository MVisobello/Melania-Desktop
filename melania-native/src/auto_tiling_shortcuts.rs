//! Physical libinput keyboard policy for COSMIC-like tiling actions.
//! No synthetic keys, host shortcuts, DRM calls or Wayland configure here.
//! Input to event() is normalized evdev (Smithay XKB keycode minus 8).
#![forbid(unsafe_code)]

const LEFT_CTRL: u32 = 29;
const RIGHT_CTRL: u32 = 97;
const LEFT_META: u32 = 125;
const RIGHT_META: u32 = 126;
const T: u32 = 20;
const Y: u32 = 21;
const O: u32 = 24;
const G: u32 = 34;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoTileKeyAction {
    Forward,
    Consume,
    Toggle,
    ToggleFloating,
    RotateLayout,
}

#[derive(Default, Debug)]
pub struct AutoTileKeyHandler {
    left_ctrl: bool,
    right_ctrl: bool,
    left_meta: bool,
    right_meta: bool,
    held: u8,
}

impl AutoTileKeyHandler {
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn event(&mut self, code: Option<u32>, pressed: bool) -> AutoTileKeyAction {
        match code {
            Some(LEFT_CTRL) => self.left_ctrl = pressed,
            Some(RIGHT_CTRL) => self.right_ctrl = pressed,
            Some(LEFT_META) => self.left_meta = pressed,
            Some(RIGHT_META) => self.right_meta = pressed,
            Some(T|Y|O|G) => {
                let super_down=self.left_meta || self.right_meta;
                let ctrl_down=self.left_ctrl || self.right_ctrl;
                let key=code.expect("matches Some");
                let (mask,valid,action) = match key {
                    T => (1,super_down && ctrl_down,AutoTileKeyAction::Toggle),
                    Y => (2,super_down && !ctrl_down,AutoTileKeyAction::Toggle),
                    O => (4,super_down && !ctrl_down,AutoTileKeyAction::RotateLayout),
                    G => (8,super_down && !ctrl_down,AutoTileKeyAction::ToggleFloating),
                    _ => unreachable!(),
                };
                if !pressed {
                    if self.held & mask != 0 {
                        self.held &= !mask;
                        return AutoTileKeyAction::Consume;
                    }
                } else if self.held & mask != 0 {
                    return AutoTileKeyAction::Consume;
                } else if valid {
                    self.held |= mask;
                    return action;
                }
            },
            _ => (),
        }
        AutoTileKeyAction::Forward
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cosmic_super_y_g_o_actions_are_one_shot() {
        let mut k=AutoTileKeyHandler::default();
        k.event(Some(LEFT_META),true);
        for (key,action) in [
            (Y,AutoTileKeyAction::Toggle),
            (G,AutoTileKeyAction::ToggleFloating),
            (O,AutoTileKeyAction::RotateLayout),
        ] {
            assert_eq!(k.event(Some(key),true),action);
            assert_eq!(k.event(Some(key),true),AutoTileKeyAction::Consume);
            assert_eq!(k.event(Some(key),false),AutoTileKeyAction::Consume);
        }
        assert_eq!(k.event(Some(Y),true),AutoTileKeyAction::Toggle);
    }
    #[test]
    fn no_super_means_no_shortcut_and_ctrl_super_t_legacy_toggle() {
        let mut k=AutoTileKeyHandler::default();
        for key in [T,Y,O,G] {
            assert_eq!(k.event(Some(key),true),AutoTileKeyAction::Forward);
            assert_eq!(k.event(Some(key),false),AutoTileKeyAction::Forward);
        }
        k.event(Some(LEFT_CTRL),true);
        assert_eq!(k.event(Some(T),true),AutoTileKeyAction::Forward);
        k.event(Some(T),false);
        k.event(Some(LEFT_META),true);
        assert_eq!(k.event(Some(T),true),AutoTileKeyAction::Toggle);
        k.event(Some(LEFT_CTRL),false);
        assert_eq!(k.event(Some(T),false),AutoTileKeyAction::Consume);
        assert_eq!(k.event(Some(Y),true),AutoTileKeyAction::Toggle);
    }
    #[test]
    fn super_ctrl_y_does_not_toggle_and_vt_reset_releases_held_key() {
        let mut k=AutoTileKeyHandler::default();
        k.event(Some(RIGHT_META),true);
        k.event(Some(RIGHT_CTRL),true);
        assert_eq!(k.event(Some(Y),true),AutoTileKeyAction::Forward);
        k.event(Some(Y),false);
        k.event(Some(RIGHT_CTRL),false);
        assert_eq!(k.event(Some(G),true),AutoTileKeyAction::ToggleFloating);
        k.reset();
        assert_eq!(k.event(Some(G),false),AutoTileKeyAction::Forward);
        assert_eq!(k.event(Some(O),true),AutoTileKeyAction::Forward);
    }
}