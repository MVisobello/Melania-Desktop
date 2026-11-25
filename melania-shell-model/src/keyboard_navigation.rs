//! Pure keyboard navigation. No GTK, Smithay or shell commands.
#![forbid(unsafe_code)]

/// Select the next visible window while maintaining the compositor's stable
/// window order. Hidden/minimized windows must be filtered by the caller.
/// Returns None when no visible window exists.
pub fn cycle_window_id(visible: &[u64], current: Option<u64>, backwards: bool) -> Option<u64> {
    if visible.is_empty() {return None;}
    let target = match current.and_then(|id| visible.iter().position(|&x| x == id)) {
        Some(position) if backwards => {
            if position == 0 {visible.len()-1} else {position-1}
        }
        Some(position) => (position + 1) % visible.len(),
        None if backwards => visible.len()-1,
        None => 0,
    };
    visible.get(target).copied()
}

/// Workspace IDs are one-based, as in Melania's IPC/taskbar.
pub fn adjacent_workspace(current: u8, count: u8, backwards: bool) -> Option<u8> {
    if count == 0 || current == 0 || current > count {return None;}
    Some(if backwards {
        if current == 1 {count} else {current-1}
    } else if current == count {1} else {current+1})
}

/// Move-and-follow only when a managed, visible, non-minimized toplevel
/// actually owns keyboard focus. A one-workspace session is a no-op.
pub fn adjacent_window_destination(current: u8, count: u8, backwards: bool,
                                   focused_visible: bool) -> Option<u8> {
    if !focused_visible { return None; }
    adjacent_workspace(current, count, backwards).filter(|dest| *dest != current)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn cycle_forward_wrap_and_unknown_focus() {
        let ids = [2, 7, 9];
        assert_eq!(cycle_window_id(&ids, Some(7), false), Some(9));
        assert_eq!(cycle_window_id(&ids, Some(9), false), Some(2));
        assert_eq!(cycle_window_id(&ids, None, false), Some(2));
        assert_eq!(cycle_window_id(&ids, Some(99), false), Some(2));
    }
    #[test] fn cycle_reverse_wrap_and_single() {
        let ids = [2, 7, 9];
        assert_eq!(cycle_window_id(&ids, Some(2), true), Some(9));
        assert_eq!(cycle_window_id(&ids, Some(7), true), Some(2));
        assert_eq!(cycle_window_id(&ids, None, true), Some(9));
        assert_eq!(cycle_window_id(&[3], Some(3), true), Some(3));
        assert_eq!(cycle_window_id(&[], None, true), None);
    }
    #[test] fn move_and_follow_requires_focus_and_distinct_valid_destination() {
        assert_eq!(adjacent_window_destination(1, 4, true, true), Some(4));
        assert_eq!(adjacent_window_destination(4, 4, false, true), Some(1));
        assert_eq!(adjacent_window_destination(2, 4, false, true), Some(3));
        assert_eq!(adjacent_window_destination(2, 4, false, false), None);
        assert_eq!(adjacent_window_destination(1, 1, true, true), None);
        assert_eq!(adjacent_window_destination(0, 4, false, true), None);
        assert_eq!(adjacent_window_destination(5, 4, true, true), None);
        assert_eq!(adjacent_window_destination(1, 0, false, true), None);
    }
    #[test] fn workspace_navigation_is_bounded_and_wraps() {
        assert_eq!(adjacent_workspace(1, 4, false), Some(2));
        assert_eq!(adjacent_workspace(4, 4, false), Some(1));
        assert_eq!(adjacent_workspace(1, 8, true), Some(8));
        assert_eq!(adjacent_workspace(3, 8, true), Some(2));
        assert_eq!(adjacent_workspace(1, 1, false), Some(1));
        assert_eq!(adjacent_workspace(0, 4, false), None);
        assert_eq!(adjacent_workspace(9, 8, true), None);
        assert_eq!(adjacent_workspace(1, 0, false), None);
    }
}
