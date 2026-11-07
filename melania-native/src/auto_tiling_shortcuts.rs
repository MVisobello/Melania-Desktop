//! Physical libinput shortcut policy for optional COSMIC-style tiling mode.
//! This model does not synthesize keys, move clients, or access the host seat.
//! Input to event() is the normalized evdev code (Smithay XKB keycode - 8).
#![forbid(unsafe_code)]

const LEFT_CTRL: u32 = 29;
const RIGHT_CTRL: u32 = 97;
const LEFT_META: u32 = 125;
const RIGHT_META: u32 = 126;
const T: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoTileKeyAction { Forward, Consume, Toggle }

#[derive(Default, Debug)]
pub struct AutoTileKeyHandler {
    left_ctrl: bool,
    right_ctrl: bool,
    left_meta: bool,
    right_meta: bool,
    t_held: bool,
}
impl AutoTileKeyHandler {
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn event(&mut self, code: Option<u32>, pressed: bool) -> AutoTileKeyAction {
        match code {
            Some(LEFT_CTRL) => self.left_ctrl = pressed,
            Some(RIGHT_CTRL) => self.right_ctrl = pressed,
            Some(LEFT_META) => self.left_meta = pressed,
            Some(RIGHT_META) => self.right_meta = pressed,
            Some(T) => {
                if !pressed {
                    if self.t_held {
                        self.t_held = false;
                        return AutoTileKeyAction::Consume;
                    }
                } else if self.t_held {
                    return AutoTileKeyAction::Consume;
                } else if (self.left_ctrl || self.right_ctrl) &&
                          (self.left_meta || self.right_meta) {
                    self.t_held = true;
                    return AutoTileKeyAction::Toggle;
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
    fn ctrl_super_t_toggles_once_per_physical_press() {
        let mut k = AutoTileKeyHandler::default();
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Forward);
        k.event(Some(T), false);
        k.event(Some(LEFT_CTRL), true);
        k.event(Some(LEFT_META), true);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Toggle);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Consume);
        k.event(Some(LEFT_CTRL), false);
        assert_eq!(k.event(Some(T), false), AutoTileKeyAction::Consume);
        k.event(Some(LEFT_CTRL), true);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Toggle);
    }
    #[test]
    fn right_modifiers_work_and_reset_revokes_a_held_press() {
        let mut k = AutoTileKeyHandler::default();
        k.event(Some(RIGHT_META), true);
        k.event(Some(RIGHT_CTRL), true);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Toggle);
        k.reset();
        assert_eq!(k.event(Some(T), false), AutoTileKeyAction::Forward);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Forward);
    }
    #[test]
    fn client_ctrl_t_and_super_t_are_never_stolen() {
        let mut k = AutoTileKeyHandler::default();
        k.event(Some(LEFT_CTRL), true);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Forward);
        k.event(Some(T), false);
        k.event(Some(LEFT_CTRL), false);
        k.event(Some(LEFT_META), true);
        assert_eq!(k.event(Some(T), true), AutoTileKeyAction::Forward);
        assert_eq!(k.event(Some(T), false), AutoTileKeyAction::Forward);
    }
}