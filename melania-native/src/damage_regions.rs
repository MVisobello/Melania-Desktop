//! Bounded, output-clipped dirty rectangles for the future native renderer.
//!
//! A dirty region is NOT a DRM ACK or permission to submit KMS. This module
//! only calculates pixels which need recomposition; the existing compositor
//! still submits full frames until a real GPU damage backend is validated.
pub const MAX_DAMAGE_RECTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    fn right(self) -> u64 {u64::from(self.x)+u64::from(self.width)}
    fn bottom(self) -> u64 {u64::from(self.y)+u64::from(self.height)}
    pub fn is_empty(self)->bool {self.width==0 || self.height==0}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageError { InvalidOutput }

#[derive(Debug)]
pub struct DamageRegions {
    output_width:u32,
    output_height:u32,
    rects:Vec<DamageRect>,
    full:bool,
    revision:u64,
}

impl DamageRegions {
    pub fn new(width:u32,height:u32)->Result<Self,DamageError> {
        if width==0 || height==0 {return Err(DamageError::InvalidOutput);}
        Ok(Self {output_width:width,output_height:height,
                 rects:Vec::new(),full:false,revision:0})
    }

    pub fn revision(&self)->u64 {self.revision}
    pub fn is_full(&self)->bool {self.full}
    pub fn is_clean(&self)->bool {!self.full && self.rects.is_empty()}
    pub fn output_size(&self)->(u32,u32) {(self.output_width,self.output_height)}
    pub fn regions(&self)->&[DamageRect] {&self.rects}

    /// Signed window geometry may be partially outside the output.
    /// All additions happen in i64; no signed i32 overflow is possible.
    pub fn clip(&self,x:i32,y:i32,width:usize,height:usize)->Option<DamageRect> {
        if width==0 || height==0 {return None;}
        let left=i64::from(x).max(0);
        let top=i64::from(y).max(0);
        let right=i64::from(x)
            .saturating_add(i64::try_from(width).unwrap_or(i64::MAX))
            .min(i64::from(self.output_width));
        let bottom=i64::from(y)
            .saturating_add(i64::try_from(height).unwrap_or(i64::MAX))
            .min(i64::from(self.output_height));
        if left>=right || top>=bottom {return None;}
        Some(DamageRect {
            x:u32::try_from(left).ok()?,
            y:u32::try_from(top).ok()?,
            width:u32::try_from(right-left).ok()?,
            height:u32::try_from(bottom-top).ok()?,
        })
    }

    fn changed(&mut self) {self.revision=self.revision.wrapping_add(1);}

    pub fn invalidate_all(&mut self) {
        if self.full {return;}
        self.full=true;
        self.rects.clear();
        self.changed();
    }

    /// Record one dirty rectangle. Touching and overlapping regions are
    /// merged until fixed point; disjoint rectangles remain independent.
    /// Too many independent changes become one full-output invalidation.
    pub fn invalidate(&mut self,x:i32,y:i32,width:usize,height:usize) {
        if self.full {return;}
        let Some(mut pending)=self.clip(x,y,width,height) else {return;};
        let mut index=0;
        while index<self.rects.len() {
            let old=self.rects[index];
            let overlaps=pending.right()>=u64::from(old.x) &&
                old.right()>=u64::from(pending.x) &&
                pending.bottom()>=u64::from(old.y) &&
                old.bottom()>=u64::from(pending.y);
            if overlaps {
                let left=pending.x.min(old.x);
                let top=pending.y.min(old.y);
                let right=pending.right().max(old.right());
                let bottom=pending.bottom().max(old.bottom());
                pending=DamageRect {
                    x:left,y:top,
                    width:u32::try_from(right-u64::from(left)).unwrap(),
                    height:u32::try_from(bottom-u64::from(top)).unwrap(),
                };
                self.rects.swap_remove(index);
                // A newly enlarged union may touch earlier rectangles.
                index=0;
            } else {
                index+=1;
            }
        }
        if self.rects.len()>=MAX_DAMAGE_RECTS {
            self.invalidate_all();
        } else {
            self.rects.push(pending);
            self.changed();
        }
    }

    /// Window move: both old and new extents must be recomposited.
    pub fn window_moved(&mut self,
        old:(i32,i32,usize,usize),new:(i32,i32,usize,usize)
    ) {
        self.invalidate(old.0,old.1,old.2,old.3);
        self.invalidate(new.0,new.1,new.2,new.3);
    }

    /// Reordering overlapping windows affects pixels even if neither
    /// client submitted new content. Conservative union of both extents.
    pub fn z_order_changed(&mut self,
        first:(i32,i32,usize,usize),second:(i32,i32,usize,usize)
    ) {
        self.invalidate(first.0,first.1,first.2,first.3);
        self.invalidate(second.0,second.1,second.2,second.3);
    }

    /// Consume after a real present. Callers must NOT clear before DRM ACK.
    pub fn clear_after_presented(&mut self) {
        if self.full || !self.rects.is_empty() {
            self.full=false;
            self.rects.clear();
            self.changed();
        }
    }

    /// A resolution change invalidates the whole new output, not just
    /// previously visible window rectangles.
    pub fn resize_output(&mut self,width:u32,height:u32)
        ->Result<(),DamageError>
    {
        if width==0 || height==0 {return Err(DamageError::InvalidOutput);}
        if self.output_width!=width || self.output_height!=height {
            self.output_width=width;
            self.output_height=height;
            self.full=false;
            self.invalidate_all();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_negative_and_extreme_coordinates_without_overflow() {
        let mut d=DamageRegions::new(100,80).unwrap();
        assert_eq!(d.clip(-10,-20,30,40),
                   Some(DamageRect{x:0,y:0,width:20,height:20}));
        assert_eq!(d.clip(90,70,usize::MAX,usize::MAX),
                   Some(DamageRect{x:90,y:70,width:10,height:10}));
        assert_eq!(d.clip(i32::MIN,i32::MIN,usize::MAX,usize::MAX),
                   Some(DamageRect{x:0,y:0,width:100,height:80}));
        assert_eq!(d.clip(110,2,10,10),None);
        assert_eq!(d.clip(0,0,0,10),None);
        d.invalidate(-10,-20,30,40);
        assert_eq!(d.regions().len(),1);
    }

    #[test]
    fn moving_and_shrinking_invalidates_both_old_and_new_extents() {
        let mut d=DamageRegions::new(1280,800).unwrap();
        d.window_moved((160,120,400,240),(200,160,320,192));
        assert_eq!(d.regions(),
            &[DamageRect{x:160,y:120,width:400,height:240}]);
        d.clear_after_presented();
        assert!(d.is_clean());
        d.window_moved((650,190,280,200),(650,190,220,180));
        assert_eq!(d.regions(),
            &[DamageRect{x:650,y:190,width:280,height:200}]);
    }

    #[test]
    fn merge_adjacent_rectangles_and_keep_disjoint_changes() {
        let mut d=DamageRegions::new(100,100).unwrap();
        d.invalidate(0,0,10,10);
        d.invalidate(30,0,10,10);
        assert_eq!(d.regions().len(),2);
        d.invalidate(10,0,20,10);
        assert_eq!(d.regions(),
            &[DamageRect{x:0,y:0,width:40,height:10}]);
        d.z_order_changed((2,20,10,10),(70,70,10,10));
        assert_eq!(d.regions().len(),3);
    }

    #[test]
    fn bounded_region_overflow_falls_back_to_full_output() {
        let mut d=DamageRegions::new(300,300).unwrap();
        for i in 0..=MAX_DAMAGE_RECTS {
            d.invalidate((i as i32)*3,0,1,1);
        }
        assert!(d.is_full());
        assert!(d.regions().is_empty());
        d.invalidate(3,3,1,1);
        assert!(d.is_full());
        d.clear_after_presented();
        assert!(d.is_clean());
    }

    #[test]
    fn output_resize_invalidates_all_and_rejects_zero_without_mutation() {
        let mut d=DamageRegions::new(100,80).unwrap();
        d.invalidate(10,10,2,2);
        assert_eq!(d.resize_output(200,160),Ok(()));
        assert!(d.is_full());
        assert_eq!(d.output_size(),(200,160));
        let before=d.revision();
        assert_eq!(d.resize_output(0,160),Err(DamageError::InvalidOutput));
        assert_eq!(d.output_size(),(200,160));
        assert_eq!(d.revision(),before);
    }
}
