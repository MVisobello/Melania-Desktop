//! Deterministic, bounded initial positions for multi-client Wayland scenes.
//! Later placement may be overridden by real client focus/drag interactions.
//! Two original QEMU window coordinates are preserved byte-for-byte.
pub const MAX_PLACED_WINDOWS:u64=16;

pub fn default_position(id:u64,width:u32,height:u32)->Option<(i32,i32)> {
    if id==0 || id>MAX_PLACED_WINDOWS ||
       width==0 || height==0 || width>16384 || height>16384 {
        return None;
    }
    let (x,y)=match id {
        1=>(160u32,120u32),
        2=>(650u32,190u32),
        _=>{
            let index=(id-3) as u32;
            (32+(index%4)*280,80+(index/4)*180)
        },
    };
    Some((x.min(width-1) as i32,y.min(height-1) as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn preserves_both_kms_pixel_proof_windows() {
        assert_eq!(default_position(1,1280,800),Some((160,120)));
        assert_eq!(default_position(2,1280,800),Some((650,190)));
    }
    #[test]
    fn sixteen_clients_get_stable_distinct_bounded_origins() {
        let positions=(1..=16).map(|id|default_position(id,1280,800).unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(positions.len(),16);
        assert!(positions.iter().all(|(x,y)|*x>=0 && *x<1280 && *y>=0 && *y<800));
    }
    #[test]
    fn rejects_invalid_and_clamps_small_outputs() {
        assert_eq!(default_position(0,1280,800),None);
        assert_eq!(default_position(17,1280,800),None);
        assert_eq!(default_position(2,0,800),None);
        assert_eq!(default_position(2,1280,0),None);
        assert_eq!(default_position(2,50000,800),None);
        assert_eq!(default_position(2,320,180),Some((319,179)));
    }
}
