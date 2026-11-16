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

/// Stable initial GPU geometry for the bounded 1280x800 acceptance scene.
/// Leave a six-pixel decoration margin and keep all visible pixels in range.
pub fn retained_gpu_position(slot:u64)->Option<(i32,i32)> {
    match slot {
        1=>Some((160,120)),
        2=>Some((650,190)),
        3=>Some((160,440)),
        4=>Some((650,500)),
        _=>default_position(slot,1280,800).map(|(x,y)|(x.clamp(6,914),y.clamp(6,570))),
    }
}

/// This is only the deterministic QEMU client fixture. Runtime admission
/// and scene composition must use actual surface IDs, never this schedule.
pub const RETAINED_GPU_PROBE_SLOTS:[u64;12]=[1,2,3,4,2,1,4,3,1,3,2,4];
pub fn retained_gpu_probe_rgb(index:u64)->Option<[u8;3]> {
    if index>=RETAINED_GPU_PROBE_SLOTS.len() as u64 {return None;}
    let delta=(index/2*10) as u8;
    Some(if index%2==0 {[84+delta,46,186]} else {[31,163-delta,69]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn retained_gpu_probe_four_independent_slots_and_non_round_robin_updates() {
        assert_eq!(RETAINED_GPU_PROBE_SLOTS.len(),12);
        for slot in 1..=4 {
            assert_eq!(RETAINED_GPU_PROBE_SLOTS.iter().filter(|&&s|s==slot).count(),3);
            let (x,y)=retained_gpu_position(slot).unwrap();
            for other in 1..slot {
                let (a,b)=retained_gpu_position(other).unwrap();
                assert!(x+366<=a-6 || a+366<=x-6 || y+230<=b-6 || b+230<=y-6);
            }
        }
        assert_ne!(RETAINED_GPU_PROBE_SLOTS[4],1);
        for id in 1..=MAX_PLACED_WINDOWS {
            let (x,y)=retained_gpu_position(id).unwrap();
            assert!(x>=6 && y>=6 && x+366<=1280 && y+230<=800);
        }
        assert_eq!(retained_gpu_position(0),None);
        assert_eq!(retained_gpu_position(17),None);
        assert_eq!(retained_gpu_probe_rgb(0),Some([84,46,186]));
        assert_eq!(retained_gpu_probe_rgb(1),Some([31,163,69]));
        assert_eq!(retained_gpu_probe_rgb(12),None);
    }
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
