//! Melania's compositor-independent window policy model.
//!
//! This is pure Rust state management, not yet a Wayland compositor.
//! Any Smithay adapter must translate protocol events into these state transitions.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Floating(Rect),
    Maximized,
    TiledLeft,
    TiledRight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub id: WindowId,
    pub app_id: String,
    pub title: String,
    pub workspace: WorkspaceId,
    pub placement: Placement,
    pub minimized: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowEvent {
    Created(WindowId),
    Focused(Option<WindowId>),
    Minimized(WindowId, bool),
    Moved(WindowId, WorkspaceId),
    PlacementChanged(WindowId, Placement),
    Closed(WindowId),
    WorkspaceChanged(WorkspaceId),
}

#[derive(Debug)]
pub struct DesktopState {
    windows: BTreeMap<WindowId, Window>,
    next_id: u64,
    active_workspace: WorkspaceId,
    focused: Option<WindowId>,
    workspace_count: u32,
}

impl DesktopState {
    pub fn new(workspace_count: u32) -> Self {
        Self {
            windows: BTreeMap::new(),
            next_id: 1,
            active_workspace: WorkspaceId(0),
            focused: None,
            workspace_count: workspace_count.max(1),
        }
    }

    pub fn windows(&self) -> &BTreeMap<WindowId, Window> { &self.windows }
    pub fn focused(&self) -> Option<WindowId> { self.focused }
    pub fn active_workspace(&self) -> WorkspaceId { self.active_workspace }
    pub fn workspace_count(&self) -> u32 { self.workspace_count }

    pub fn create_window(&mut self, app_id: impl Into<String>, title: impl Into<String>, rect: Rect) -> (WindowId, Vec<WindowEvent>) {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        self.windows.insert(id, Window {
            id,
            app_id: app_id.into(),
            title: title.into(),
            workspace: self.active_workspace,
            placement: Placement::Floating(rect),
            minimized: false,
        });
        self.focused = Some(id);
        (id, vec![WindowEvent::Created(id), WindowEvent::Focused(Some(id))])
    }

    pub fn focus(&mut self, id: WindowId) -> Option<WindowEvent> {
        let w = self.windows.get(&id)?;
        if w.minimized || w.workspace != self.active_workspace { return None; }
        self.focused = Some(id);
        Some(WindowEvent::Focused(Some(id)))
    }

    pub fn set_minimized(&mut self, id: WindowId, minimized: bool) -> Vec<WindowEvent> {
        let Some(w) = self.windows.get_mut(&id) else { return Vec::new(); };
        if w.minimized == minimized { return Vec::new(); }
        w.minimized = minimized;
        let mut out = vec![WindowEvent::Minimized(id, minimized)];
        if self.focused == Some(id) && minimized {
            self.focused = None;
            out.push(WindowEvent::Focused(None));
        }
        out
    }

    pub fn set_placement(&mut self, id: WindowId, placement: Placement) -> Option<WindowEvent> {
        let w = self.windows.get_mut(&id)?;
        w.placement = placement;
        Some(WindowEvent::PlacementChanged(id, placement))
    }

    pub fn move_to_workspace(&mut self, id: WindowId, workspace: WorkspaceId) -> Vec<WindowEvent> {
        if workspace.0 >= self.workspace_count { return Vec::new(); }
        let Some(w) = self.windows.get_mut(&id) else { return Vec::new(); };
        w.workspace = workspace;
        let mut out = vec![WindowEvent::Moved(id, workspace)];
        if self.focused == Some(id) && workspace != self.active_workspace {
            self.focused = None;
            out.push(WindowEvent::Focused(None));
        }
        out
    }

    pub fn switch_workspace(&mut self, workspace: WorkspaceId) -> Vec<WindowEvent> {
        if workspace.0 >= self.workspace_count || workspace == self.active_workspace { return Vec::new(); }
        self.active_workspace = workspace;
        self.focused = None;
        vec![WindowEvent::WorkspaceChanged(workspace), WindowEvent::Focused(None)]
    }

    pub fn close_window(&mut self, id: WindowId) -> Vec<WindowEvent> {
        if self.windows.remove(&id).is_none() { return Vec::new(); }
        let mut out = vec![WindowEvent::Closed(id)];
        if self.focused == Some(id) {
            self.focused = None;
            out.push(WindowEvent::Focused(None));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Rect = Rect { x: 10, y: 15, width: 800, height: 600 };

    #[test]
    fn creates_focused_windows_with_unique_ids() {
        let mut state = DesktopState::new(4);
        let (a, events) = state.create_window("org.test.Files", "Files", BOX);
        let (b, _) = state.create_window("org.test.Browser", "Browser", BOX);
        assert_ne!(a, b);
        assert_eq!(state.focused(), Some(b));
        assert_eq!(events, vec![WindowEvent::Created(a), WindowEvent::Focused(Some(a))]);
    }

    #[test]
    fn never_focuses_a_hidden_or_minimized_window() {
        let mut state = DesktopState::new(2);
        let (a, _) = state.create_window("org.test", "Example", BOX);
        state.set_minimized(a, true);
        assert_eq!(state.focus(a), None);
        state.set_minimized(a, false);
        assert!(state.focus(a).is_some());
        state.switch_workspace(WorkspaceId(1));
        assert_eq!(state.focus(a), None);
    }

    #[test]
    fn moving_a_focused_window_to_another_workspace_clears_focus() {
        let mut state = DesktopState::new(2);
        let (a, _) = state.create_window("org.test", "Example", BOX);
        state.move_to_workspace(a, WorkspaceId(1));
        assert_eq!(state.focused(), None);
        state.switch_workspace(WorkspaceId(1));
        assert!(state.focus(a).is_some());
    }

    #[test]
    fn rejects_out_of_range_workspace() {
        let mut state = DesktopState::new(2);
        assert!(state.switch_workspace(WorkspaceId(5)).is_empty());
        assert_eq!(state.active_workspace(), WorkspaceId(0));
    }

    #[test]
    fn snap_and_close_window() {
        let mut state = DesktopState::new(1);
        let (id, _) = state.create_window("org.test", "Test", BOX);
        assert_eq!(state.set_placement(id, Placement::TiledLeft), Some(WindowEvent::PlacementChanged(id, Placement::TiledLeft)));
        assert_eq!(state.windows()[&id].placement, Placement::TiledLeft);
        state.close_window(id);
        assert_eq!(state.focused(), None);
        assert!(state.windows().is_empty());
    }
}

pub mod interaction;
