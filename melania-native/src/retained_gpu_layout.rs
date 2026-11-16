//! Retained GPU scene geometry, separate from the underlying GPU texture.
//! Coordinates are physical pixels in the isolated 1280x800 output.
//! Slot IDs come from the Wayland registry, not the client.
use std::collections::BTreeMap;
use crate::window_placement::{retained_gpu_position, MAX_PLACED_WINDOWS};

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
    pub fn raise(&mut self,slot:u64)->Result<bool,&'static str> {
        let index=self.stack.iter().position(|id|*id==slot).ok_or("unknown GPU surface")?;
        if index+1==self.stack.len() {return Ok(false);}
        self.stack.remove(index);
        self.stack.push(slot);
        Ok(true)
    }
    pub fn hit_test(&self,x:f64,y:f64)->Option<u64> {
        if !x.is_finite() || !y.is_finite() {return None;}
        self.stack.iter().rev().copied().find(|slot|self.rects.get(slot).is_some_and(|r|r.contains(x,y)))
    }
    pub fn remove(&mut self,slot:u64)->bool {
        if self.rects.remove(&slot).is_none(){return false;}
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
    fn invalid_update_does_not_mutate_existing_window_or_order() {
        let mut scene=RetainedGpuLayout::default();
        let original=scene.update_buffer(1,360,224).unwrap();
        assert!(scene.update_buffer(1,0,224).is_err());
        assert_eq!(scene.geometry(1),Some(original));
        assert_eq!(scene.bottom_to_top().count(),1);
    }
}
