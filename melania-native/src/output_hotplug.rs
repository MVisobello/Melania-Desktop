//! Atomic monitor-topology updates. A failed refresh leaves prior routing
//! intact, avoiding half-applied hotplug configurations.
use crate::output_layout::{OutputLayout,DisplayRect,LayoutError};
use std::collections::BTreeSet;

#[derive(Debug,Default)]
pub struct OutputTopology {
    layout:OutputLayout,
    epoch:u64,
}
impl OutputTopology {
    pub fn epoch(&self)->u64 {self.epoch}
    pub fn layout(&self)->&OutputLayout {&self.layout}
    /// Empty list represents unplugged displays; duplicate connector IDs,
    /// overlap or invalid geometry reject the ENTIRE proposed topology.
    pub fn replace_all(&mut self,proposed:&[DisplayRect])->Result<bool,LayoutError>{
        let mut fresh=OutputLayout::default();
        let mut ids=BTreeSet::new();
        for output in proposed {
            if !ids.insert(output.id) {return Err(LayoutError::InvalidOutput);}
            fresh.set(*output)?;
        }
        let mut before=self.layout.outputs().to_vec();
        let mut after=fresh.outputs().to_vec();
        before.sort_by_key(|d|d.id);
        after.sort_by_key(|d|d.id);
        if before==after {return Ok(false);}
        self.layout=fresh;
        self.epoch=self.epoch.wrapping_add(1);
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn d(id:u32,x:i32)->DisplayRect {DisplayRect{id,x,y:0,width:400,height:300}}
    #[test]
    fn bad_hotplug_never_partially_changes_active_routing() {
        let mut t=OutputTopology::default();
        assert_eq!(t.replace_all(&[d(1,0),d(2,400)]),Ok(true));
        let old=t.epoch();
        assert_eq!(t.replace_all(&[d(1,0),d(2,300)]),
                   Err(LayoutError::OverlappingOutputs));
        assert_eq!(t.epoch(),old);
        assert_eq!(t.layout().route(450.0,0.0),Some((2,50.0,0.0)));
        assert_eq!(t.replace_all(&[d(1,0),d(1,400)]),Err(LayoutError::InvalidOutput));
        assert_eq!(t.epoch(),old);
    }
    #[test]
    fn reordered_identical_displays_do_not_invalidate_frames() {
        let mut t=OutputTopology::default();
        t.replace_all(&[d(1,0),d(2,400)]).unwrap();
        let old=t.epoch();
        assert_eq!(t.replace_all(&[d(2,400),d(1,0)]),Ok(false));
        assert_eq!(t.epoch(),old);
        assert_eq!(t.replace_all(&[]),Ok(true));
        assert!(t.layout().outputs().is_empty());
    }
}
