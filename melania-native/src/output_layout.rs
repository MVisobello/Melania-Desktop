//! Safe layout model for up to eight real/virtual compositor outputs.
//! This model does NOT set KMS modes, create monitor resources or own DRM.
pub const MAX_LAYOUT_OUTPUTS:usize=8;
pub const MAX_SIDE:u32=16384;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct DisplayRect {
    pub id:u32,pub x:i32,pub y:i32,pub width:u32,pub height:u32,
}
impl DisplayRect {
    pub fn valid(self)->bool {
        if self.id==0 || self.width==0 || self.height==0 ||
           self.width>MAX_SIDE || self.height>MAX_SIDE {return false;}
        let right=i64::from(self.x)+i64::from(self.width);
        let bottom=i64::from(self.y)+i64::from(self.height);
        right<=i64::from(i32::MAX) && bottom<=i64::from(i32::MAX)
    }
    pub fn contains(self,x:f64,y:f64)->bool {
        x.is_finite() && y.is_finite() &&
        x>=f64::from(self.x) && y>=f64::from(self.y) &&
        x<f64::from(self.x)+f64::from(self.width) &&
        y<f64::from(self.y)+f64::from(self.height)
    }
    fn intersects(self,other:Self)->bool {
        let a=(i64::from(self.x),i64::from(self.y),
               i64::from(self.x)+i64::from(self.width),
               i64::from(self.y)+i64::from(self.height));
        let b=(i64::from(other.x),i64::from(other.y),
               i64::from(other.x)+i64::from(other.width),
               i64::from(other.y)+i64::from(other.height));
        a.0<b.2 && b.0<a.2 && a.1<b.3 && b.1<a.3
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LayoutError {InvalidOutput,DuplicateOutput,OverlappingOutputs,CapacityExceeded}
#[derive(Debug,Default)]
pub struct OutputLayout {
    outputs:Vec<DisplayRect>,revision:u64,
}
impl OutputLayout {
    pub fn revision(&self)->u64 {self.revision}
    pub fn outputs(&self)->&[DisplayRect] {&self.outputs}
    pub fn get(&self,id:u32)->Option<DisplayRect> {
        self.outputs.iter().copied().find(|o|o.id==id)
    }
    /// Atomically insert or replace; reject ambiguous screen overlap.
    pub fn set(&mut self,rect:DisplayRect)->Result<(),LayoutError> {
        if !rect.valid() {return Err(LayoutError::InvalidOutput);}
        let previous=self.outputs.iter().position(|o|o.id==rect.id);
        if previous.is_none() && self.outputs.len()>=MAX_LAYOUT_OUTPUTS {
            return Err(LayoutError::CapacityExceeded);
        }
        if self.outputs.iter().any(|o|o.id!=rect.id && o.intersects(rect)) {
            return Err(LayoutError::OverlappingOutputs);
        }
        match previous {
            Some(index)=>{if self.outputs[index]==rect {return Ok(());}
                          self.outputs[index]=rect;},
            None=>self.outputs.push(rect),
        }
        self.revision=self.revision.wrapping_add(1);
        Ok(())
    }
    /// Apply one whole DRM connector/layout generation atomically. A pair
    /// of real monitors may swap positions (or a dock may add/remove several
    /// outputs at once); sequential set() calls would reject the intermediate
    /// overlap although the final layout is valid. In case of a stale, duplicate,
    /// overlapping or oversized snapshot the old routing stays untouched.
    ///
    /// Empty layouts are permitted for VT pause/all-disconnected: callers
    /// MUST treat them as no drawable surface, never invent an output.
    pub fn reconfigure(
        &mut self, requested:impl IntoIterator<Item=DisplayRect>
    )->Result<bool,LayoutError> {
        let mut next=Vec::new();
        for rect in requested {
            if next.len()>=MAX_LAYOUT_OUTPUTS {
                return Err(LayoutError::CapacityExceeded);
            }
            if !rect.valid() {
                return Err(LayoutError::InvalidOutput);
            }
            if next.iter().any(|old:&DisplayRect|old.id==rect.id) {
                return Err(LayoutError::DuplicateOutput);
            }
            if next.iter().any(|old:&DisplayRect|old.intersects(rect)) {
                return Err(LayoutError::OverlappingOutputs);
            }
            next.push(rect);
        }
        next.sort_unstable_by_key(|rect|rect.id);
        let mut old=self.outputs.clone();
        old.sort_unstable_by_key(|rect|rect.id);
        if old==next {return Ok(false);}
        self.outputs=next;
        self.revision=self.revision.wrapping_add(1);
        Ok(true)
    }
    pub fn remove(&mut self,id:u32)->bool {
        if let Some(index)=self.outputs.iter().position(|o|o.id==id) {
            self.outputs.remove(index);
            self.revision=self.revision.wrapping_add(1);
            true
        } else {false}
    }
    /// Return real output ID and local physical coordinates.
    pub fn route(&self,x:f64,y:f64)->Option<(u32,f64,f64)> {
        let screen=self.outputs.iter().find(|r|r.contains(x,y))?;
        Some((screen.id,x-f64::from(screen.x),y-f64::from(screen.y)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn a(id:u32,x:i32,y:i32,w:u32,h:u32)->DisplayRect {
        DisplayRect{id,x,y,width:w,height:h}
    }
    #[test]
    fn two_displays_left_and_right_route_coordinates_and_edges() {
        let mut m=OutputLayout::default();
        m.set(a(1,0,0,1280,800)).unwrap();
        m.set(a(2,1280,0,1920,1080)).unwrap();
        assert_eq!(m.route(1279.0,799.0),Some((1,1279.0,799.0)));
        assert_eq!(m.route(1280.0,0.0),Some((2,0.0,0.0)));
        assert_eq!(m.route(3199.0,1079.0),Some((2,1919.0,1079.0)));
        assert_eq!(m.route(3200.0,0.0),None);
        assert_eq!(m.route(f64::NAN,1.0),None);
    }
    #[test]
    fn rejects_overlaps_and_overflows_without_mutation() {
        let mut m=OutputLayout::default();
        m.set(a(1,0,0,100,100)).unwrap();
        let revision=m.revision();
        assert_eq!(m.set(a(2,99,0,100,100)),Err(LayoutError::OverlappingOutputs));
        assert_eq!(m.set(a(2,i32::MAX-5,0,100,10)),Err(LayoutError::InvalidOutput));
        assert_eq!(m.set(a(1,0,0,0,100)),Err(LayoutError::InvalidOutput));
        assert_eq!(m.revision(),revision);
        assert_eq!(m.get(1),Some(a(1,0,0,100,100)));
        assert!(!m.remove(0));
    }
    #[test]
    fn atomic_monitor_swap_and_hotplug_no_transient_overlap() {
        let mut m=OutputLayout::default();
        assert_eq!(m.reconfigure([
            a(1,0,0,1920,1080),a(2,1920,0,1280,800)
        ]),Ok(true));
        let first=m.revision();
        // A sequential update would temporarily overlap the other output.
        assert_eq!(m.set(a(1,1280,0,1920,1080)),Err(LayoutError::OverlappingOutputs));
        assert_eq!(m.reconfigure([
            a(2,0,0,1280,800),a(1,1280,0,1920,1080)
        ]),Ok(true));
        assert_eq!(m.revision(),first+1);
        assert_eq!(m.route(1279.0,100.0),Some((2,1279.0,100.0)));
        assert_eq!(m.route(1280.0,100.0),Some((1,0.0,100.0)));
        let epoch=m.revision();
        assert_eq!(m.reconfigure([
            a(1,1280,0,1920,1080),a(2,0,0,1280,800)
        ]),Ok(false));
        assert_eq!(m.revision(),epoch);
        assert_eq!(m.reconfigure([a(2,0,0,1280,800)]),Ok(true));
        assert_eq!(m.route(1280.0,100.0),None);
        assert_eq!(m.reconfigure([]),Ok(true));
        assert_eq!(m.route(100.0,100.0),None);
    }
    #[test]
    fn atomic_reconfigure_rejects_bad_topology_without_losing_active_outputs() {
        let mut m=OutputLayout::default();
        m.reconfigure([a(1,0,0,1280,800)]).unwrap();
        let original=m.outputs().to_vec();
        let epoch=m.revision();
        let invalid=[
            (vec![a(1,0,0,1280,800),a(1,1280,0,1920,1080)],LayoutError::DuplicateOutput),
            (vec![a(1,0,0,1280,800),a(2,1279,0,1920,1080)],LayoutError::OverlappingOutputs),
            (vec![a(1,0,0,1280,800),a(2,1280,0,0,1080)],LayoutError::InvalidOutput),
            ((1..=9).map(|id|a(id,(id as i32-1)*100,0,100,100)).collect(),LayoutError::CapacityExceeded),
        ];
        for (snapshot,error) in invalid {
            assert_eq!(m.reconfigure(snapshot),Err(error));
            assert_eq!(m.outputs(),original.as_slice());
            assert_eq!(m.revision(),epoch);
        }
    }
    #[test]
    fn hotplug_replacement_and_capacity_are_bounded() {
        let mut m=OutputLayout::default();
        for i in 1..=8 {m.set(a(i,(i as i32-1)*100,0,100,100)).unwrap();}
        assert_eq!(m.outputs().len(),MAX_LAYOUT_OUTPUTS);
        assert_eq!(m.set(a(9,800,0,100,100)),Err(LayoutError::CapacityExceeded));
        assert!(m.remove(4));
        m.set(a(9,800,0,100,100)).unwrap();
        assert_eq!(m.route(800.0,1.0),Some((9,0.0,1.0)));
        let old=m.revision();
        m.set(a(9,800,0,100,100)).unwrap();
        assert_eq!(m.revision(),old);
    }
}
