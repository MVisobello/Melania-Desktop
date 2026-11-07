//! COSMIC-style tab stacks for the native tiled window manager.
//!
//! This is a bounded compositor policy. It never unmaps a wl_surface,
//! sends XDG configure, steals focus, or certifies KMS/VT ownership. The
//! Wayland bridge must atomically apply visible tab selections with real
//! scene damage and must ignore hidden tabs for input and screen capture.
#![forbid(unsafe_code)]

pub const MAX_STACK_WINDOWS: usize = 32;
pub const MAX_TABS_PER_STACK: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabError {
    InvalidId,
    DuplicateWindow,
    UnknownWindow,
    NotStacking,
    StackFull,
    Capacity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Group {
    tabs: Vec<u64>,
    active: u64,
    stacking: bool,
}

impl Group {
    fn contains(&self, id: u64) -> bool { self.tabs.contains(&id) }
}

#[derive(Clone, Debug, Default)]
pub struct TabGroups {
    groups: Vec<Group>,
}

impl TabGroups {
    pub fn groups_count(&self) -> usize { self.groups.len() }
    pub fn window_count(&self) -> usize {
        self.groups.iter().map(|g|g.tabs.len()).sum()
    }
    pub fn register(&mut self, id: u64) -> Result<(), TabError> {
        if id==0 {return Err(TabError::InvalidId);}
        if self.groups.iter().any(|g|g.contains(id)) {
            return Err(TabError::DuplicateWindow);
        }
        if self.window_count()>=MAX_STACK_WINDOWS {return Err(TabError::Capacity);}
        self.groups.push(Group{tabs:vec![id],active:id,stacking:false});
        Ok(())
    }
    pub fn is_stacking(&self,id:u64)->Result<bool,TabError>{
        self.groups.iter().find(|g|g.contains(id))
            .map(|g|g.stacking).ok_or(TabError::UnknownWindow)
    }
    pub fn group_tabs(&self,id:u64)->Result<&[u64],TabError>{
        self.groups.iter().find(|g|g.contains(id))
            .map(|g|g.tabs.as_slice()).ok_or(TabError::UnknownWindow)
    }
    pub fn active_tab(&self,id:u64)->Result<u64,TabError>{
        self.groups.iter().find(|g|g.contains(id))
            .map(|g|g.active).ok_or(TabError::UnknownWindow)
    }
    pub fn visible(&self,id:u64)->Result<bool,TabError>{
        self.groups.iter().find(|g|g.contains(id))
            .map(|g|g.active==id).ok_or(TabError::UnknownWindow)
    }
    pub fn visible_ids(&self)->Vec<u64> {
        self.groups.iter().map(|g|g.active).collect()
    }

    /// Super+S creates a one-tab stack; toggling it again removes the
    /// chrome/grouping. On a multi-tab stack this splits all members into
    /// independent windows, preserving their current order and identities.
    pub fn toggle_stacking(&mut self,id:u64)->Result<bool,TabError>{
        let i=self.groups.iter().position(|g|g.contains(id))
            .ok_or(TabError::UnknownWindow)?;
        if !self.groups[i].stacking {
            self.groups[i].stacking=true;
            return Ok(true);
        }
        let old=self.groups.remove(i);
        for (offset,tab) in old.tabs.into_iter().enumerate() {
            self.groups.insert(i+offset,Group{
                tabs:vec![tab],active:tab,stacking:false
            });
        }
        Ok(false)
    }

    /// Stack a source window onto a stack target. The source's former
    /// group stays valid even when it was active; empty groups disappear.
    /// No state is mutated if the destination is invalid or full.
    pub fn attach(&mut self,id:u64,target:u64)->Result<bool,TabError>{
        if id==target{return Err(TabError::InvalidId);}
        let source_index=self.groups.iter().position(|g|g.contains(id))
            .ok_or(TabError::UnknownWindow)?;
        let target_index=self.groups.iter().position(|g|g.contains(target))
            .ok_or(TabError::UnknownWindow)?;
        if source_index==target_index {
            self.groups[source_index].active=id;
            return Ok(false);
        }
        if !self.groups[target_index].stacking {return Err(TabError::NotStacking);}
        if self.groups[target_index].tabs.len()>=MAX_TABS_PER_STACK {
            return Err(TabError::StackFull);
        }
        {
            let group=&mut self.groups[source_index];
            group.tabs.retain(|&tab|tab!=id);
            if group.active==id {
                if let Some(&replacement)=group.tabs.first() {group.active=replacement;}
            }
        }
        if self.groups[source_index].tabs.is_empty() {
            self.groups.remove(source_index);
        }
        let target_index=self.groups.iter().position(|g|g.contains(target))
            .expect("target survives a distinct group removal");
        self.groups[target_index].tabs.push(id);
        self.groups[target_index].active=id;
        Ok(true)
    }

    /// Remove a tab from a group without losing its window identity.
    pub fn detach(&mut self,id:u64)->Result<bool,TabError>{
        let i=self.groups.iter().position(|g|g.contains(id))
            .ok_or(TabError::UnknownWindow)?;
        if self.groups[i].tabs.len()==1 {
            if self.groups[i].stacking {
                self.groups[i].stacking=false;
                return Ok(true);
            }
            return Ok(false);
        }
        let group=&mut self.groups[i];
        group.tabs.retain(|&tab|tab!=id);
        if group.active==id {group.active=group.tabs[0];}
        self.groups.insert(i+1,Group{
            tabs:vec![id],active:id,stacking:false
        });
        Ok(true)
    }

    pub fn activate(&mut self,id:u64)->Result<bool,TabError>{
        let group=self.groups.iter_mut().find(|g|g.contains(id))
            .ok_or(TabError::UnknownWindow)?;
        let changed=group.active!=id;
        group.active=id;
        Ok(changed)
    }
    /// Cycle only within the selected stack and return the new active tab.
    /// A singleton or normal unstacked window never changes selection.
    pub fn cycle(&mut self,id:u64,backwards:bool)->Result<Option<u64>,TabError>{
        let group=self.groups.iter_mut().find(|g|g.contains(id))
            .ok_or(TabError::UnknownWindow)?;
        if !group.stacking || group.tabs.len()<2 {return Ok(None);}
        let i=group.tabs.iter().position(|&tab|tab==group.active)
            .expect("active member must belong to its group");
        let next=if backwards {
            (i+group.tabs.len()-1)%group.tabs.len()
        } else {(i+1)%group.tabs.len()};
        group.active=group.tabs[next];
        Ok(Some(group.active))
    }

    pub fn forget(&mut self,id:u64)->bool {
        let Some(i)=self.groups.iter().position(|g|g.contains(id)) else{return false;};
        let group=&mut self.groups[i];
        group.tabs.retain(|&tab|tab!=id);
        if group.tabs.is_empty() {
            self.groups.remove(i);
        } else if group.active==id {
            group.active=group.tabs[0];
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn groups(ids:&[u64])->TabGroups{
        let mut t=TabGroups::default();
        for &id in ids {t.register(id).unwrap();}
        t
    }
    #[test]
    fn one_window_super_s_stack_and_unstack_is_stable() {
        let mut t=groups(&[4]);
        assert_eq!(t.toggle_stacking(4),Ok(true));
        assert_eq!(t.visible_ids(),vec![4]);
        assert_eq!(t.toggle_stacking(4),Ok(false));
        assert_eq!(t.group_tabs(4),Ok(&[4][..]));
    }
    #[test]
    fn attach_and_cycle_preserve_order_and_show_only_selected_tab() {
        let mut t=groups(&[1,2,3]);
        assert_eq!(t.toggle_stacking(1),Ok(true));
        assert_eq!(t.attach(2,1),Ok(true));
        assert_eq!(t.attach(3,1),Ok(true));
        assert_eq!(t.groups_count(),1);
        assert_eq!(t.group_tabs(1),Ok(&[1,2,3][..]));
        assert_eq!(t.visible_ids(),vec![3]);
        assert_eq!(t.visible(2),Ok(false));
        assert_eq!(t.cycle(1,false),Ok(Some(1)));
        assert_eq!(t.cycle(2,true),Ok(Some(3)));
        assert_eq!(t.activate(2),Ok(true));
        assert_eq!(t.visible_ids(),vec![2]);
        assert_eq!(t.activate(2),Ok(false));
    }
    #[test]
    fn detaching_active_tab_preserves_previous_stack() {
        let mut t=groups(&[1,2,3]);
        t.toggle_stacking(1).unwrap();
        t.attach(2,1).unwrap();
        t.attach(3,1).unwrap();
        assert_eq!(t.detach(3),Ok(true));
        assert_eq!(t.group_tabs(1),Ok(&[1,2][..]));
        assert_eq!(t.visible_ids(),vec![1,3]);
        assert!(!t.is_stacking(3).unwrap());
        assert_eq!(t.window_count(),3);
    }
    #[test]
    fn turning_off_multitab_stack_keeps_all_clients_and_order() {
        let mut t=groups(&[10,20,30]);
        t.toggle_stacking(10).unwrap();
        t.attach(20,10).unwrap();
        t.attach(30,10).unwrap();
        assert_eq!(t.toggle_stacking(20),Ok(false));
        assert_eq!(t.visible_ids(),vec![10,20,30]);
        assert_eq!(t.groups_count(),3);
        assert_eq!(t.window_count(),3);
    }
    #[test]
    fn teardown_of_active_tab_selects_surviving_client() {
        let mut t=groups(&[1,2]);
        t.toggle_stacking(1).unwrap();
        t.attach(2,1).unwrap();
        assert!(t.forget(2));
        assert_eq!(t.visible_ids(),vec![1]);
        assert!(t.forget(1));
        assert!(t.visible_ids().is_empty());
        assert!(!t.forget(1));
    }
    #[test]
    fn invalid_groups_and_source_remove_rejected_without_mutation() {
        let mut t=groups(&[1,2,3]);
        let original=t.visible_ids();
        assert_eq!(t.attach(2,1),Err(TabError::NotStacking));
        assert_eq!(t.attach(1,1),Err(TabError::InvalidId));
        assert_eq!(t.attach(7,1),Err(TabError::UnknownWindow));
        assert_eq!(t.attach(2,7),Err(TabError::UnknownWindow));
        assert_eq!(t.toggle_stacking(7),Err(TabError::UnknownWindow));
        assert_eq!(t.activate(7),Err(TabError::UnknownWindow));
        assert_eq!(t.visible_ids(),original);
    }
    #[test]
    fn bounded_capacity_and_duplicate_ids() {
        let mut t=groups(&(1..=MAX_STACK_WINDOWS as u64).collect::<Vec<_>>());
        assert_eq!(t.register(33),Err(TabError::Capacity));
        assert_eq!(t.register(1),Err(TabError::DuplicateWindow));
        assert_eq!(t.register(0),Err(TabError::InvalidId));
        t.toggle_stacking(1).unwrap();
        for id in 2..=MAX_TABS_PER_STACK as u64 {
            assert_eq!(t.attach(id,1),Ok(true));
        }
        assert_eq!(t.attach(9,1),Err(TabError::StackFull));
        assert_eq!(t.window_count(),MAX_STACK_WINDOWS);
    }
    #[test]
    fn owned_scene_pixels_and_pointer_follow_active_tab_only() {
        use crate::dynamic_window_scene::{
            DynamicWindowScene, WindowPixels, DynamicAlpha
        };
        let solid=|color:[u8;4]|{
            let mut pixels=vec![0;32*24*4];
            for part in pixels.chunks_exact_mut(4){part.copy_from_slice(&color);}
            WindowPixels::new(32,24,DynamicAlpha::Opaque,pixels).unwrap()
        };
        let mut groups=groups(&[1,2]);
        let mut scene=DynamicWindowScene::new(96,64,[1,2,3,0]).unwrap();
        scene.upsert(1,4,4,solid([9,20,30,0])).unwrap();
        scene.upsert(2,52,4,solid([70,80,90,0])).unwrap();
        let budget=scene.used_bytes();
        let before=scene.generation();
        assert!(scene.confirm_presented_damage(before));
        groups.toggle_stacking(1).unwrap();
        groups.attach(2,1).unwrap();
        scene.move_to(2,4,4).unwrap();
        assert_eq!(groups.visible(1),Ok(false));
        assert_eq!(scene.set_visible(1,false),Ok(true));
        assert_eq!(scene.hit_test(10.0,10.0),Some(2));
        assert_eq!(scene.hit_test(60.0,10.0),None);
        let current=scene.composite().unwrap();
        assert_eq!(&current[(10*96+10)*4..(10*96+10)*4+3], &[70,80,90]);
        assert_eq!(groups.cycle(2,false),Ok(Some(1)));
        assert!(scene.set_visible(2,false).unwrap());
        assert!(scene.set_visible(1,true).unwrap());
        assert_eq!(scene.hit_test(10.0,10.0),Some(1));
        let first=scene.composite().unwrap();
        assert_eq!(&first[(10*96+10)*4..(10*96+10)*4+3], &[9,20,30]);
        assert_eq!(scene.used_bytes(),budget);
        assert!(!scene.confirm_presented_damage(before));
        assert!(scene.confirm_presented_damage(scene.generation()));
    }
    #[test]
    fn active_tab_closure_keeps_surviving_real_window_visible() {
        use crate::dynamic_window_scene::{
            DynamicWindowScene, WindowPixels, DynamicAlpha
        };
        let make=||WindowPixels::new(12,12,DynamicAlpha::Opaque,
            vec![50;12*12*4]).unwrap();
        let mut groups=groups(&[1,2]);
        let mut scene=DynamicWindowScene::new(60,40,[1,1,1,0]).unwrap();
        scene.upsert(1,4,4,make()).unwrap();
        scene.upsert(2,4,4,make()).unwrap();
        groups.toggle_stacking(1).unwrap();
        groups.attach(2,1).unwrap();
        scene.set_visible(1,false).unwrap();
        assert_eq!(scene.hit_test(6.0,6.0),Some(2));
        assert!(groups.forget(2));
        assert!(scene.remove(2));
        assert_eq!(groups.visible(1),Ok(true));
        assert!(scene.set_visible(1,true).unwrap());
        assert_eq!(scene.hit_test(6.0,6.0),Some(1));
        assert_eq!(scene.window_count(),1);
        assert_eq!(scene.used_bytes(),12*12*4);
    }

    #[test]
    fn joining_from_active_stack_repairs_source_selection() {
        let mut t=groups(&[1,2,3,4]);
        t.toggle_stacking(1).unwrap();
        t.toggle_stacking(3).unwrap();
        t.attach(2,1).unwrap();
        assert_eq!(t.visible_ids(),vec![2,3,4]);
        t.attach(2,3).unwrap();
        assert_eq!(t.visible_ids(),vec![1,2,4]);
        assert_eq!(t.group_tabs(3),Ok(&[3,2][..]));
        assert_eq!(t.window_count(),4);
    }
}
