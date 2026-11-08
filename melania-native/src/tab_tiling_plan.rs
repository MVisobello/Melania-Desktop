//! COSMIC-style tab-stack tile layout adapter (policy only).
//!
//! One *stack* consumes one tiled rectangle, with every retained XDG
//! client still registered. The active tab is the slot representative.
//! No XDG configure, wl_shm modification or DRM page flip occurs here.
//! A future bridge must apply a complete group resize transaction and
//! accept every client's matching committed buffer before making it visible.
#![forbid(unsafe_code)]

use crate::{
    auto_tiling::{AutoTiling,TileError},
    tab_groups::{TabGroups,TabError},
    window_interaction::WindowRect,
};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct StackTileSlot {
    /// The only visible tab and the member used to reserve one tile.
    pub active:u64,
    /// All live client IDs, including inactive tabs whose buffers remain owned.
    pub members:Vec<u64>,
    pub rect:WindowRect,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum StackTileError {
    /// A group references a client not registered with the tiling manager.
    MissingTilerMember(u64),
    /// A tiling placement has no matching, registered tab group.
    MissingTabGroup(u64),
    Tab(TabError),
    Tile(TileError),
}

pub fn plan_tab_stacks(
    tiler:&AutoTiling, groups:&TabGroups,
    work:WindowRect, width:i32, height:i32,
)->Result<Option<Vec<StackTileSlot>>,StackTileError> {
    if !tiler.enabled(){return Ok(None);}
    // Layout on a local clone. Hidden group members can never consume a tile,
    // and a rejected layout must never mutate the production tiler.
    let mut candidate=tiler.clone();
    for active in groups.visible_ids() {
        let members=groups.group_tabs(active).map_err(StackTileError::Tab)?;
        for &member in members {
            if tiler.is_floating(member).is_err(){
                return Err(StackTileError::MissingTilerMember(member));
            }
            if member!=active {
                candidate.set_role_excluded(member,true)
                    .map_err(StackTileError::Tile)?;
            }
        }
    }
    let Some(plan)=candidate.plan(work,width,height)
        .map_err(StackTileError::Tile)? else{return Ok(None);};
    let mut slots=Vec::with_capacity(plan.len());
    for placement in plan {
        let members=groups.group_tabs(placement.window_id)
            .map_err(|_|StackTileError::MissingTabGroup(placement.window_id))?;
        // The role's owned pixels must still be checked against rect size by
        // the bridge on actual XDG configure/ACK/client-commit transactions.
        slots.push(StackTileSlot{
            active:placement.window_id,members:members.to_vec(),
            rect:placement.rect,
        });
    }
    Ok(Some(slots))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup()->(AutoTiling,TabGroups) {
        let mut tiles=AutoTiling::default();
        tiles.set_enabled(true);
        let mut tabs=TabGroups::default();
        for id in 1..=3 {
            tiles.register(id).unwrap();
            tabs.register(id).unwrap();
        }
        tabs.toggle_stacking(1).unwrap();
        tabs.attach(2,1).unwrap();
        (tiles,tabs)
    }
    fn work()->WindowRect {
        WindowRect{x:0,y:0,width:960,height:720}
    }
    #[test]
    fn two_tabs_consume_one_tile_and_keep_both_owned_clients(){
        let (tiler,groups)=setup();
        let plan=plan_tab_stacks(&tiler,&groups,work(),960,720)
            .unwrap().unwrap();
        assert_eq!(plan.len(),2);
        assert_eq!(plan[0].active,2);
        assert_eq!(plan[0].members,vec![1,2]);
        assert_eq!(plan[1].active,3);
        assert_eq!(plan[1].members,vec![3]);
        assert!(plan[0].rect.x+plan[0].rect.width<=plan[1].rect.x);
        assert_eq!(tiler.member_count(),3);
        assert_eq!(groups.window_count(),3);
        // Hidden tab 1 was excluded only in the cloned policy.
        assert_eq!(tiler.plan(work(),960,720).unwrap().unwrap().len(),3);
    }
    #[test]
    fn switching_active_tab_preserves_group_placement_and_client_identity(){
        let (tiler,mut groups)=setup();
        let before=plan_tab_stacks(&tiler,&groups,work(),960,720)
            .unwrap().unwrap();
        assert_eq!(groups.cycle(2,false),Ok(Some(1)));
        let after=plan_tab_stacks(&tiler,&groups,work(),960,720)
            .unwrap().unwrap();
        assert_eq!(after[0].active,1);
        assert_eq!(after[0].members,vec![1,2]);
        assert_eq!(after[0].rect,before[0].rect);
        assert_eq!(after[1].rect,before[1].rect);
    }
    #[test]
    fn floating_and_excluded_active_stack_do_not_consume_hidden_extra_slot(){
        let (mut tiler,groups)=setup();
        tiler.set_floating(2,true).unwrap();
        let plan=plan_tab_stacks(&tiler,&groups,work(),960,720)
            .unwrap().unwrap();
        assert_eq!(plan.len(),1);
        assert_eq!(plan[0].members,vec![3]);
        assert_eq!(plan[0].rect,work());
        tiler.set_floating(2,false).unwrap();
        tiler.set_role_excluded(2,true).unwrap();
        let plan=plan_tab_stacks(&tiler,&groups,work(),960,720)
            .unwrap().unwrap();
        assert_eq!(plan.len(),1);
    }
    #[test]
    fn errors_are_atomic_and_disabled_tiling_is_a_noop(){
        let (mut tiler,mut groups)=setup();
        let original=tiler.plan(work(),960,720).unwrap().unwrap();
        assert_eq!(plan_tab_stacks(&tiler,&groups,
            WindowRect{x:0,y:0,width:150,height:90},150,90),
            Err(StackTileError::Tile(TileError::CannotFit)));
        assert_eq!(tiler.plan(work(),960,720).unwrap().unwrap(),original);
        tiler.forget(1);
        assert_eq!(plan_tab_stacks(&tiler,&groups,work(),960,720),
            Err(StackTileError::MissingTilerMember(1)));
        tiler.register(1).unwrap();
        groups.forget(3);
        assert_eq!(plan_tab_stacks(&tiler,&groups,work(),960,720),
            Err(StackTileError::MissingTabGroup(3)));
        tiler.set_enabled(false);
        assert_eq!(plan_tab_stacks(&tiler,&groups,work(),960,720),Ok(None));
    }
}
