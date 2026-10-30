//! Melania's pure workspace assignment policy.
//! The compositor owns mapping/focus; this crate never issues Wayland calls.
use std::collections::BTreeMap;

pub const DEFAULT_WORKSPACES: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError { InvalidWorkspace, UnknownWindow, DuplicateWindow }

/// Workspace IDs are 1-based in UI and IPC. Windows stay in their assigned
/// workspace even when minimized. A switch must not silently unminimize them.
#[derive(Debug, Clone)]
pub struct Workspaces {
    active: u8,
    count: u8,
    assigned: BTreeMap<u64, u8>,
}
impl Default for Workspaces { fn default() -> Self { Self::new(DEFAULT_WORKSPACES) } }
impl Workspaces {
    pub fn new(count: u8) -> Self {
        assert!((1..=16).contains(&count), "workspace count must be 1..=16");
        Self { active: 1, count, assigned: BTreeMap::new() }
    }
    pub fn current(&self) -> u8 { self.active }
    pub fn count(&self) -> u8 { self.count }
    pub fn workspace_of(&self, id: u64) -> Option<u8> { self.assigned.get(&id).copied() }
    pub fn is_visible(&self, id: u64) -> bool { self.workspace_of(id) == Some(self.active) }
    pub fn register(&mut self, id: u64) -> Result<(), WorkspaceError> {
        if id == 0 || self.assigned.contains_key(&id) { return Err(WorkspaceError::DuplicateWindow); }
        self.assigned.insert(id, self.active);
        Ok(())
    }
    pub fn forget(&mut self, id: u64) -> bool { self.assigned.remove(&id).is_some() }
    pub fn switch(&mut self, id: u8) -> Result<bool, WorkspaceError> {
        if id == 0 || id > self.count { return Err(WorkspaceError::InvalidWorkspace); }
        let changed = self.active != id;
        self.active = id;
        Ok(changed)
    }
    pub fn send(&mut self, window_id: u64, dest: u8) -> Result<bool, WorkspaceError> {
        if dest == 0 || dest > self.count { return Err(WorkspaceError::InvalidWorkspace); }
        let old = self.assigned.get_mut(&window_id).ok_or(WorkspaceError::UnknownWindow)?;
        let changed = *old != dest;
        *old = dest;
        Ok(changed)
    }
    pub fn windows_in(&self, workspace: u8) -> impl Iterator<Item=u64> + '_ {
        self.assigned.iter().filter(move |(_, ws)| **ws == workspace).map(|(id,_)|*id)
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn starts_on_first_workspace() {
        let mut s=Workspaces::default();assert_eq!(s.current(),1);
        s.register(12).unwrap();assert!(s.is_visible(12));
        assert!(!s.switch(1).unwrap());
        assert!(s.switch(2).unwrap());assert!(!s.is_visible(12));
        assert_eq!(s.workspace_of(12),Some(1));
    }
    #[test] fn send_does_not_change_current_workspace() {
        let mut s=Workspaces::new(4);s.register(1).unwrap();
        s.send(1,3).unwrap();assert_eq!(s.current(),1);
        assert!(!s.is_visible(1));s.switch(3).unwrap();assert!(s.is_visible(1));
    }
    #[test] fn invalid_workspaces_are_rejected() {
        let mut s=Workspaces::default();s.register(7).unwrap();
        assert_eq!(s.switch(0),Err(WorkspaceError::InvalidWorkspace));
        assert_eq!(s.switch(5),Err(WorkspaceError::InvalidWorkspace));
        assert_eq!(s.send(7,5),Err(WorkspaceError::InvalidWorkspace));
        assert_eq!(s.send(99,2),Err(WorkspaceError::UnknownWindow));
        assert_eq!(s.current(),1);assert_eq!(s.workspace_of(7),Some(1));
    }
    #[test] fn register_and_forget_window() {
        let mut s=Workspaces::default();s.register(8).unwrap();
        assert_eq!(s.register(8),Err(WorkspaceError::DuplicateWindow));
        assert!(s.forget(8));assert!(!s.forget(8));
        assert_eq!(s.workspace_of(8),None);s.register(8).unwrap();
    }
    #[test] fn move_multiple_and_filter() {
        let mut s=Workspaces::new(3);
        for id in 1..=5 {s.register(id).unwrap();}
        s.send(2,2).unwrap();s.send(4,2).unwrap();
        assert_eq!(s.windows_in(2).collect::<Vec<_>>(),vec![2,4]);
        assert_eq!(s.windows_in(1).collect::<Vec<_>>(),vec![1,3,5]);
    }
}

pub mod popup;
