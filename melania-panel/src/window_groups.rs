//! Pure taskbar grouping policy: no GTK or compositor side effects.
//! Keeps existing window order and never merges anonymous application IDs.
use melania_control::WindowInfo;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowGroup {
    pub app_id: String,
    pub windows: Vec<WindowInfo>,
}

impl WindowGroup {
    pub fn active(&self) -> bool { self.windows.iter().any(|w| w.active) }
    pub fn all_minimized(&self) -> bool { self.windows.iter().all(|w| w.minimized) }
}

/// Stable app grouping, including safe fallbacks for empty/unspecified app IDs.
/// `enabled=false` is the historical behavior: a button per real window.
/// Grouping is a presentation policy only; IPC still targets exact window IDs.
pub fn group_windows(windows: &[WindowInfo], enabled: bool) -> Vec<WindowGroup> {
    let mut groups: Vec<WindowGroup> = Vec::new();
    let mut indices: HashMap<String, usize> = HashMap::new();
    for window in windows {
        let app = window.app_id.trim();
        let key = if enabled && !app.is_empty() {
            format!("app:{}", app.to_lowercase())
        } else {
            format!("window:{}", window.id)
        };
        if let Some(&idx) = indices.get(&key) {
            groups[idx].windows.push(window.clone());
        } else {
            indices.insert(key, groups.len());
            groups.push(WindowGroup {
                app_id: window.app_id.clone(),
                windows: vec![window.clone()],
            });
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(id:u64, app:&str, active:bool, minimized:bool)->WindowInfo {
        WindowInfo {id, title:format!("Window {id}"), app_id:app.into(), active, minimized}
    }
    #[test]
    fn off_preserves_every_window_and_order() {
        let input=[w(1,"org.test.App",false,false),w(2,"org.test.App",true,false)];
        let groups=group_windows(&input,false);
        assert_eq!(groups.len(),2);
        assert_eq!(groups[0].windows[0].id,1);
        assert_eq!(groups[1].windows[0].id,2);
    }
    #[test]
    fn groups_case_insensitively_and_preserves_first_occurrence() {
        let input=[w(1,"org.App",false,true),w(2,"Other",false,false),
                   w(3,"ORG.APP",true,false)];
        let groups=group_windows(&input,true);
        assert_eq!(groups.len(),2);
        assert_eq!(groups[0].windows.iter().map(|w|w.id).collect::<Vec<_>>(),vec![1,3]);
        assert!(groups[0].active());
        assert!(!groups[0].all_minimized());
        assert!(!groups[1].active());
    }
    #[test]
    fn anonymous_windows_remain_separate() {
        let groups=group_windows(&[w(1,"",false,true),w(2," ",false,true)],true);
        assert_eq!(groups.len(),2);
        assert!(groups.iter().all(WindowGroup::all_minimized));
    }
    #[test]
    fn empty_input_is_empty() {
        assert!(group_windows(&[],true).is_empty());
    }
}
