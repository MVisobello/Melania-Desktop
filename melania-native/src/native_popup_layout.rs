//! Bounded XDG popup placement for the actual native Wayland retained scene.
//! A role configure is NOT display evidence: only client-owned wl_shm pixels
//! committed at its ACKed size can enter the KMS scene. This module contains
//! no login, DRM or libseat privileges.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct PopupPlacement {
    pub parent_key:u64,
    pub relative_x:i32,
    pub relative_y:i32,
    pub width:i32,
    pub height:i32,
}

impl PopupPlacement {
    pub fn new(parent_key:u64,relative_x:i32,relative_y:i32,width:i32,height:i32)
        ->Option<Self>
    {
        if parent_key==0 || !(1..=8192).contains(&width) ||
           !(1..=8192).contains(&height) {return None;}
        Some(Self{parent_key,relative_x,relative_y,width,height})
    }

    /// The parent may move *after* xdg_popup.configure: use the current
    /// retained parent placement to follow it, never old screen coordinates.
    /// Negative coordinates are legal for a partially clipped popup.
    pub fn absolute(self,parent_x:i32,parent_y:i32,output_w:i32,output_h:i32)
        ->Option<(i32,i32)>
    {
        if output_w<=0 || output_h<=0 {return None;}
        let x=i64::from(parent_x)+i64::from(self.relative_x);
        let y=i64::from(parent_y)+i64::from(self.relative_y);
        let r=x+i64::from(self.width);
        let b=y+i64::from(self.height);
        // The popup must have at least one visible pixel, preventing
        // invisible hit targets and unbounded offscreen scenes.
        if x>=i64::from(output_w) || y>=i64::from(output_h) ||
           r<=0 || b<=0 || x<i64::from(i32::MIN) ||
           y<i64::from(i32::MIN) || x>i64::from(i32::MAX) ||
           y>i64::from(i32::MAX) {return None;}
        Some((x as i32,y as i32))
    }

    /// Supply a drawable position only after the caller has established the
    /// ACKed XDG popup role and its mapped parent. This is the validation
    /// gate BEFORE allocating the client's finite retained scene slot.
    pub fn accepted_buffer(self, w:usize, h:usize, parent_x:i32, parent_y:i32,
                           output_w:i32, output_h:i32)->Option<(i32,i32)> {
        if !self.matches_buffer(w,h) {return None;}
        self.absolute(parent_x,parent_y,output_w,output_h)
    }

    /// Reject wrong-sized popup commits after an XDG configure or reposition.
    pub fn matches_buffer(self,w:usize,h:usize)->bool {
        w==self.width as usize && h==self.height as usize
    }
}


/// Deterministic descendant-before-parent teardown of the actual XDG popup
/// graph. The bridge supplies only its registered Wayland surface identities.
///
/// No recursion: malformed parent cycles and duplicated relationships must
/// neither overflow the compositor's stack nor evict an unrelated tree.
/// Work and allocations are bounded by the finite registered role list.
pub fn popup_teardown_order<T:Clone+PartialEq>(
    root:&T, relationships:&[(T,T)]
)->Vec<T> {
    let mut seen=Vec::with_capacity(relationships.len().saturating_add(1));
    let mut order=Vec::with_capacity(relationships.len().saturating_add(1));
    let mut pending=vec![(root.clone(),false)];
    while let Some((current,expanded))=pending.pop() {
        if expanded {
            order.push(current);
            continue;
        }
        if seen.contains(&current) {continue;}
        seen.push(current.clone());
        pending.push((current.clone(),true));
        for (parent,child) in relationships.iter().rev() {
            if parent==&current && !seen.contains(child) {
                pending.push((child.clone(),false));
            }
        }
    }
    order
}

/// Remove only descendant popup roles, never their live parent toplevel.
/// Used when a toplevel is minimized or destroyed; preserving the root role
/// identity during minimize prevents an accidental client disconnect.
pub fn popup_descendant_teardown_order<T:Clone+PartialEq>(
    parent:&T,relationships:&[(T,T)]
)->Vec<T> {
    let mut order=popup_teardown_order(parent,relationships);
    order.retain(|candidate|candidate!=parent);
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_tracks_parent_drags_without_inventing_a_new_surface_buffer() {
        let popup=PopupPlacement::new(1,40,30,210,160).unwrap();
        assert_eq!(popup.absolute(160,120,1280,800),Some((200,150)));
        assert_eq!(popup.absolute(420,260,1280,800),Some((460,290)));
        assert!(popup.matches_buffer(210,160));
        assert!(!popup.matches_buffer(211,160));
        assert!(!popup.matches_buffer(210,159));
    }
    #[test]
    fn popup_is_clipped_by_real_output_and_refuses_overflow_or_unmapped_roles() {
        let popup=PopupPlacement::new(3,-55,-30,80,50).unwrap();
        assert_eq!(popup.absolute(20,18,1280,800),Some((-35,-12)));
        assert_eq!(popup.absolute(-1000,-1000,1280,800),None);
        // A popup can remain partly visible even when its parent origin is
        // at the output edge; only reject it once its own left edge is offscreen.
        assert_eq!(popup.absolute(1280,2,1280,800),Some((1225,-28)));
        assert_eq!(popup.absolute(1335,2,1280,800),None);
        assert_eq!(popup.absolute(100,120,0,800),None);
        assert!(PopupPlacement::new(0,0,0,40,40).is_none());
        assert!(PopupPlacement::new(1,0,0,0,40).is_none());
        assert!(PopupPlacement::new(1,0,0,8193,40).is_none());
        let extreme=PopupPlacement::new(1,i32::MAX,i32::MIN,80,50).unwrap();
        assert!(extreme.absolute(500,500,1280,800).is_none());
    }
    #[test]
    fn no_scene_slot_for_wrong_size_or_offscreen_popup_buffer() {
        let popup=PopupPlacement::new(9,20,-10,128,72).unwrap();
        assert_eq!(popup.accepted_buffer(128,72,40,35,1280,800),Some((60,25)));
        assert_eq!(popup.accepted_buffer(127,72,40,35,1280,800),None);
        assert_eq!(popup.accepted_buffer(128,71,40,35,1280,800),None);
        assert_eq!(popup.accepted_buffer(128,72,-1000,35,1280,800),None);
        assert_eq!(popup.accepted_buffer(128,72,40,35,0,800),None);
        assert_eq!(popup.accepted_buffer(128,72,i32::MAX,35,1280,800),None);
    }
    #[test]
    fn nested_or_repositioned_popups_have_separate_parent_epochs() {
        let root=PopupPlacement::new(1,180,30,240,140).unwrap();
        let sub=PopupPlacement::new(2,210,0,170,90).unwrap();
        let parent=root.absolute(300,100,1280,800).unwrap();
        assert_eq!(sub.absolute(parent.0,parent.1,1280,800),Some((690,130)));
        let repositioned=PopupPlacement::new(1,20,50,240,140).unwrap();
        assert_eq!(repositioned.absolute(300,100,1280,800),Some((320,150)));
    }
}

/// Select the deepest mapped popup in the currently focused XDG parent chain.
///
/// No unrelated, hidden or uncommitted role may capture Escape. The input
/// bridge supplies only actual registered/mapped role keys; this pure policy
/// does not synthesize a grab or keyboard event.
pub fn escape_popup_target(
    focused:u64, children:&[(u64,u64,bool)]
)->Option<u64> {
    if focused==0 {return None;}
    let mut current=focused;
    let mut visited=vec![focused];
    let mut selected=children.iter().any(|(_,child,mapped)|
        *child==focused && *mapped).then_some(focused);
    for _ in 0..8 {
        let Some((_,child,_))=children.iter().rev().find(|(parent,child,mapped)|
            *parent==current && *child!=current && *mapped
        ) else {break;};
        // A malformed/cyclic parent chain must not hijack input.
        if visited.contains(child) {return None;}
        visited.push(*child);
        current=*child;
        selected=Some(current);
    }
    selected
}

#[cfg(test)]
mod popup_escape_tests {
    use super::{escape_popup_target,popup_teardown_order,popup_descendant_teardown_order};

    #[test]
    fn deepest_mapped_child_only_under_focused_parent() {
        let roles=[(1,2,true),(1,4,true),(2,3,true),(5,6,true)];
        assert_eq!(escape_popup_target(1,&roles),Some(4));
        assert_eq!(escape_popup_target(2,&roles),Some(3));
        assert_eq!(escape_popup_target(5,&roles),Some(6));
        assert_eq!(escape_popup_target(7,&roles),None);
        assert_eq!(escape_popup_target(0,&roles),None);
    }
    #[test]
    fn hidden_or_unacknowledged_roles_never_intercept_escape() {
        let roles=[(1,2,true),(2,3,false),(5,6,true)];
        assert_eq!(escape_popup_target(1,&roles),Some(2));
        assert_eq!(escape_popup_target(2,&roles),Some(2));
        assert_eq!(escape_popup_target(3,&roles),None);
        assert_eq!(escape_popup_target(4,&roles),None);
        assert_eq!(escape_popup_target(1,&[(1,2,false)]),None);
    }
    #[test]
    fn nested_popup_chain_is_bounded_even_on_corrupt_parent_cycles() {
        let roles=[(1,2,true),(2,3,true),(3,2,true)];
        assert_eq!(escape_popup_target(1,&roles),None);
    }

    #[test]
    fn postorder_evicts_nested_popups_before_ancestors() {
        // Root 1 owns two menus; menu 2 also owns a submenu.
        assert_eq!(popup_teardown_order(&1,&[(1,2),(2,3),(1,4)]),
                   vec![3,2,4,1]);
        assert_eq!(popup_teardown_order(&7,&[(1,2),(2,3),(1,4)]),
                   vec![7]);
    }
    #[test]
    fn cyclic_or_duplicate_roles_cannot_recurse_or_evict_other_apps() {
        let malformed=[(1,2),(2,3),(3,1),(1,2),(5,6)];
        let order=popup_teardown_order(&1,&malformed);
        assert_eq!(order,vec![3,2,1]);
        assert_eq!(order.iter().filter(|item|**item==2).count(),1);
        assert!(!order.contains(&5));
        assert!(!order.contains(&6));
        assert_eq!(popup_teardown_order(&5,&malformed),vec![6,5]);
    }
    #[test]
    fn parent_minimize_clears_all_nested_menus_but_preserves_toplevel() {
        let tree=[(1,2),(2,3),(1,4),(8,9),(3,2)];
        assert_eq!(popup_descendant_teardown_order(&1,&tree),vec![3,2,4]);
        assert_eq!(popup_descendant_teardown_order(&8,&tree),vec![9]);
        assert!(popup_descendant_teardown_order(&9,&tree).is_empty());
        assert!(!popup_descendant_teardown_order(&1,&tree).contains(&1));
    }
    #[test]
    fn self_parent_and_diamond_teardown_visit_each_surface_once() {
        let edges=[(1,1),(1,2),(1,3),(2,4),(3,4)];
        let order=popup_teardown_order(&1,&edges);
        assert_eq!(order.len(),4);
        assert_eq!(order.last(),Some(&1));
        for role in 1..=4 {
            assert_eq!(order.iter().filter(|item|**item==role).count(),1);
        }
    }

}
