//! Retained GPU scene geometry, separate from the underlying GPU texture.
//! Coordinates are physical pixels in the isolated 1280x800 output.
//! Slot IDs come from the Wayland registry, not the client.
use std::collections::{BTreeMap,BTreeSet};
use crate::window_placement::{retained_gpu_position, MAX_PLACED_WINDOWS};
use crate::window_interaction::{ResizeDirection,WindowRect};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct GpuRect { pub x:i32, pub y:i32, pub width:i32, pub height:i32 }

impl GpuRect {
    fn fit(x:i32,y:i32,width:i32,height:i32)->Option<Self> {
        if !(1..=1268).contains(&width) || !(1..=788).contains(&height) {return None;}
        Some(Self{x:x.clamp(6,1274-width),y:y.clamp(6,794-height),width,height})
    }
    fn contains(self,x:f64,y:f64)->bool {
        x>=f64::from(self.x) && y>=f64::from(self.y) &&
        x<f64::from(self.x+self.width) && y<f64::from(self.y+self.height)
    }
}
#[derive(Debug,Default)]
pub struct RetainedGpuLayout {
    rects:BTreeMap<u64,GpuRect>,
    hidden:BTreeSet<u64>,
    // Bottom to top. Buffer replacement never changes this order.
    stack:Vec<u64>,
}
impl RetainedGpuLayout {
    pub fn update_buffer(&mut self,slot:u64,width:i32,height:i32)->Result<GpuRect,&'static str> {
        if !(1..=MAX_PLACED_WINDOWS).contains(&slot) {return Err("invalid GPU surface slot");}
        let prior=self.rects.get(&slot).copied();
        let (x,y)=prior.map(|r|(r.x,r.y)).or_else(||retained_gpu_position(slot))
            .ok_or("missing GPU initial placement")?;
        let rect=GpuRect::fit(x,y,width,height).ok_or("invalid GPU buffer geometry")?;
        if prior.is_none() {
            // Preserve the established stable slot ordering for newly
            // mapped clients, without changing the order of existing ones.
            self.stack.push(slot);
        }
        self.rects.insert(slot,rect);
        Ok(rect)
    }
    pub fn geometry(&self,slot:u64)->Option<GpuRect> {self.rects.get(&slot).copied()}
    pub fn move_to(&mut self,slot:u64,x:i32,y:i32)->Result<bool,&'static str> {
        let old=self.geometry(slot).ok_or("unknown GPU surface")?;
        let next=GpuRect::fit(x,y,old.width,old.height).ok_or("invalid GPU move")?;
        if old==next {return Ok(false);}
        self.rects.insert(slot,next);
        Ok(true)
    }
    /// Clamp only the edge being dragged, reserving the existing six-pixel
    /// decoration margin. Opposite edges must remain anchored on all eight
    /// XDG directions. This is a preview, not a published GPU geometry.
    pub fn resize_target(&self,slot:u64,requested:WindowRect,direction:ResizeDirection)
        ->Option<GpuRect> {
        if self.is_visible(slot)!=Some(true) {return None;}
        let old=self.geometry(slot)?;
        let (mut left,mut top)=(old.x,old.y);
        let (mut right,mut bottom)=(old.x+old.width,old.y+old.height);
        if direction.left() {
            if right-96<6 {return None;}
            left=requested.x.clamp(6,right-96);
        } else if direction.right() {
            if left+96>1274 {return None;}
            right=requested.x.checked_add(requested.width)?.clamp(left+96,1274);
        }
        if direction.top() {
            if bottom-72<6 {return None;}
            top=requested.y.clamp(6,bottom-72);
        } else if direction.bottom() {
            if top+72>794 {return None;}
            bottom=requested.y.checked_add(requested.height)?.clamp(top+72,794);
        }
        let next=GpuRect{x:left,y:top,width:right-left,height:bottom-top};
        (next!=old && GpuRect::fit(next.x,next.y,next.width,next.height)==Some(next))
            .then_some(next)
    }

    /// Commit the configured origin and size together. A wrong-sized buffer,
    /// hidden/missing role or off-screen geometry never alters the scene.
    /// Both Smithay and the retained GLES presenter use the same rule.
    pub fn commit_resize(&mut self,slot:u64,target:GpuRect,width:i32,height:i32)
        ->Result<(),&'static str> {
        if self.is_visible(slot)!=Some(true) || (target.width,target.height)!=(width,height) ||
            width<96 || height<72 || GpuRect::fit(target.x,target.y,width,height)!=Some(target) {
            return Err("invalid configured GPU resize");
        }
        self.rects.insert(slot,target);
        Ok(())
    }

    pub fn raise(&mut self,slot:u64)->Result<bool,&'static str> {
        if self.is_visible(slot)!=Some(true) {return Err("cannot raise hidden or unknown GPU surface");}
        let index=self.stack.iter().position(|id|*id==slot).ok_or("unknown GPU surface")?;
        if index+1==self.stack.len() {return Ok(false);}
        self.stack.remove(index);
        self.stack.push(slot);
        Ok(true)
    }
    pub fn is_visible(&self,slot:u64)->Option<bool> {
        self.rects.contains_key(&slot).then(||!self.hidden.contains(&slot))
    }
    pub fn set_visible(&mut self,slot:u64,visible:bool)->Result<bool,&'static str> {
        if !self.rects.contains_key(&slot) {return Err("unknown GPU surface");}
        Ok(if visible {self.hidden.remove(&slot)} else {self.hidden.insert(slot)})
    }
    pub fn hit_test(&self,x:f64,y:f64)->Option<u64> {
        if !x.is_finite() || !y.is_finite() {return None;}
        self.stack.iter().rev().copied().find(|slot|!self.hidden.contains(slot) && self.rects.get(slot).is_some_and(|r|r.contains(x,y)))
    }
    /// Use the same visible stack for selection and surface-local input origin.
    pub fn target_at(&self,x:f64,y:f64)->Option<(u64,GpuRect)> {
        let slot=self.hit_test(x,y)?;
        Some((slot,self.geometry(slot)?))
    }
    pub fn remove(&mut self,slot:u64)->bool {
        if self.rects.remove(&slot).is_none(){return false;}
        self.hidden.remove(&slot);
        self.stack.retain(|id|*id!=slot);
        true
    }
    pub fn bottom_to_top(&self)->impl Iterator<Item=(u64,GpuRect)>+'_ {
        self.stack.iter().map(|slot|(*slot,self.rects[slot]))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_survive_real_buffer_dimension_changes_and_clamp_to_output() {
        let mut scene=RetainedGpuLayout::default();
        scene.update_buffer(1,360,224).unwrap();
        assert_eq!(scene.move_to(1,440,300),Ok(true));
        assert_eq!(scene.move_to(1,440,300),Ok(false));
        assert_eq!(scene.update_buffer(1,400,240).unwrap(),GpuRect{x:440,y:300,width:400,height:240});
        assert_eq!(scene.move_to(1,i32::MAX,i32::MIN),Ok(true));
        assert_eq!(scene.geometry(1).unwrap(),GpuRect{x:874,y:6,width:400,height:240});
        assert_eq!(scene.update_buffer(1,500,250).unwrap().x,774);
        assert!(scene.update_buffer(1,i32::MAX,250).is_err());
        assert_eq!(scene.geometry(1).unwrap().width,500);
    }
    #[test]
    fn overlap_raise_buffer_replacement_and_close_keep_consistent_hit_test() {
        let mut scene=RetainedGpuLayout::default();
        scene.update_buffer(1,300,200).unwrap();
        scene.update_buffer(2,300,200).unwrap();
        scene.move_to(2,160,120).unwrap();
        assert_eq!(scene.hit_test(200.0,150.0),Some(2));
        assert_eq!(scene.raise(1),Ok(true));
        scene.update_buffer(2,320,220).unwrap();
        assert_eq!(scene.hit_test(200.0,150.0),Some(1));
        assert_eq!(scene.bottom_to_top().map(|(id,_)|id).collect::<Vec<_>>(),vec![2,1]);
        assert!(scene.remove(1));
        assert!(!scene.remove(1));
        assert_eq!(scene.hit_test(200.0,150.0),Some(2));
        assert_eq!(scene.hit_test(f64::NAN,150.0),None);
        assert!(scene.move_to(1,0,0).is_err());
        assert!(scene.raise(1).is_err());
        assert!(scene.update_buffer(17,10,10).is_err());
    }
    #[test]
    fn hidden_texture_keeps_geometry_and_stack_but_cannot_receive_pointer() {
        let mut scene=RetainedGpuLayout::default();
        scene.update_buffer(1,360,224).unwrap();
        scene.update_buffer(2,320,200).unwrap();
        scene.move_to(2,160,120).unwrap();
        assert_eq!(scene.hit_test(200.0,150.0),Some(2));
        assert_eq!(scene.set_visible(2,false),Ok(true));
        assert_eq!(scene.set_visible(2,false),Ok(false));
        assert_eq!(scene.hit_test(200.0,150.0),Some(1));
        assert!(scene.raise(2).is_err());
        let rect=scene.update_buffer(2,340,210).unwrap();
        assert_eq!(scene.is_visible(2),Some(false));
        assert_eq!(scene.geometry(2),Some(rect));
        assert_eq!(scene.bottom_to_top().count(),2);
        assert_eq!(scene.set_visible(2,true),Ok(true));
        assert_eq!(scene.hit_test(200.0,150.0),Some(2));
        scene.set_visible(2,false).unwrap();
        assert!(scene.remove(2));
        assert_eq!(scene.is_visible(2),None);
        scene.update_buffer(2,320,200).unwrap();
        assert_eq!(scene.is_visible(2),Some(true));
        assert!(scene.set_visible(99,false).is_err());
    }
    #[test]
    fn input_target_tracks_moved_raised_and_hidden_geometry() {
        let mut scene=RetainedGpuLayout::default();
        scene.update_buffer(1,360,224).unwrap();
        scene.update_buffer(2,320,200).unwrap();
        scene.move_to(1,600,300).unwrap();
        scene.move_to(2,650,330).unwrap();
        let point=(700.0,400.0);
        let (slot,rect)=scene.target_at(point.0,point.1).unwrap();
        assert_eq!((slot,point.0-f64::from(rect.x),point.1-f64::from(rect.y)),(2,50.0,70.0));
        scene.raise(1).unwrap();
        let (slot,rect)=scene.target_at(point.0,point.1).unwrap();
        assert_eq!((slot,point.0-f64::from(rect.x),point.1-f64::from(rect.y)),(1,100.0,100.0));
        scene.set_visible(1,false).unwrap();
        assert_eq!(scene.target_at(point.0,point.1).unwrap().0,2);
        scene.remove(2);
        assert!(scene.target_at(point.0,point.1).is_none());
        assert!(scene.target_at(f64::NAN,0.0).is_none());
    }
    #[test]
    fn all_eight_resize_edges_anchor_opposite_edges_and_commit_exact_buffer() {
        use crate::window_interaction::{WindowInteraction,InteractionKind};
        for edge in [ResizeDirection::Top,ResizeDirection::Bottom,
            ResizeDirection::Left,ResizeDirection::Right,ResizeDirection::TopLeft,
            ResizeDirection::TopRight,ResizeDirection::BottomLeft,ResizeDirection::BottomRight] {
            let mut scene=RetainedGpuLayout::default();
            let old=scene.update_buffer(1,360,224).unwrap();
            scene.update_buffer(2,320,200).unwrap();
            scene.raise(1).unwrap();
            let mut drag=WindowInteraction::default();
            let start=WindowRect{x:old.x,y:old.y,width:old.width,height:old.height};
            assert!(drag.begin(1,start,200.0,150.0,InteractionKind::Resize(edge)));
            let (_,request)=drag.motion(240.0,180.0).unwrap();
            let next=scene.resize_target(1,request,edge).unwrap();
            assert_eq!(scene.geometry(1),Some(old));
            assert_eq!(next.x,old.x+if edge.left(){40}else{0});
            assert_eq!(next.y,old.y+if edge.top(){30}else{0});
            assert_eq!(next.width,old.width+if edge.right(){40}else if edge.left(){-40}else{0});
            assert_eq!(next.height,old.height+if edge.bottom(){30}else if edge.top(){-30}else{0});
            assert!(scene.commit_resize(1,next,next.width-1,next.height).is_err());
            assert_eq!(scene.geometry(1),Some(old));
            scene.commit_resize(1,next,next.width,next.height).unwrap();
            assert_eq!(scene.geometry(1),Some(next));
            assert_eq!(scene.bottom_to_top().map(|(id,_)|id).collect::<Vec<_>>(),vec![2,1]);
        }
    }
    #[test]
    fn resize_clamps_moving_edges_to_margin_and_rejects_stale_roles() {
        let mut scene=RetainedGpuLayout::default();
        let old=scene.update_buffer(1,360,224).unwrap();
        let requested=WindowRect{x:0,y:0,width:old.x+old.width,height:old.y+old.height};
        let next=scene.resize_target(1,requested,ResizeDirection::TopLeft).unwrap();
        assert_eq!(next.x,6);
        assert_eq!(next.y,6);
        assert_eq!(next.x+next.width,old.x+old.width);
        assert_eq!(next.y+next.height,old.y+old.height);
        assert!(scene.commit_resize(1,GpuRect{x:0,..next},next.width,next.height).is_err());
        scene.set_visible(1,false).unwrap();
        assert!(scene.resize_target(1,requested,ResizeDirection::TopLeft).is_none());
        assert!(scene.commit_resize(1,next,next.width,next.height).is_err());
        scene.remove(1);
        assert!(scene.commit_resize(1,next,next.width,next.height).is_err());
    }
    #[test]
    fn invalid_update_does_not_mutate_existing_window_or_order() {
        let mut scene=RetainedGpuLayout::default();
        let original=scene.update_buffer(1,360,224).unwrap();
        assert!(scene.update_buffer(1,0,224).is_err());
        assert_eq!(scene.geometry(1),Some(original));
        assert_eq!(scene.bottom_to_top().count(),1);
    }
}
