//! Deterministic partial CPU composition for verified dirty rectangles.
//! The KMS frame transport remains full-size until tested separately.
//! Every rectangle is clipped/validated BEFORE mutating the target.
//! Original Wayland premultiplied-alpha blending is delegated to scene::compose.
use crate::damage_regions::DamageRect;
use crate::scene::{self,Pixels,Surface,SceneError};

fn translate<'a>(s:&Surface<'a>,x:u32,y:u32)->Surface<'a> {
    let pixels=match &s.pixels {
        Pixels::OpaqueBgra{data,stride}=>Pixels::OpaqueBgra{data,stride:*stride},
        Pixels::StraightAlphaBgra{data,stride}=>Pixels::StraightAlphaBgra{data,stride:*stride},
        Pixels::PremultipliedAlphaBgra{data,stride}=>
            Pixels::PremultipliedAlphaBgra{data,stride:*stride},
        Pixels::Solid(c)=>Pixels::Solid(*c),
    };
    let shift=|origin:i32,off:u32| {
        (i64::from(origin)-i64::from(off))
            .clamp(i64::from(i32::MIN),i64::from(i32::MAX)) as i32
    };
    Surface {x:shift(s.x,x),y:shift(s.y,y),
             width:s.width,height:s.height,layer:s.layer,pixels}
}

/// Patch only the given fully visible dirty output rectangles. Never assumes
/// dirty rectangle disjointness: repeated patches are deterministic.
/// A blank region leaves previously presented bytes untouched.
pub fn compose_regions_into(
    width:usize,height:usize,background:[u8;4],
    surfaces:&[Surface<'_>],regions:&[DamageRect],
    target:&mut [u8],
)->Result<(),SceneError> {
    let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(4))
        .ok_or(SceneError::InvalidDimensions)?;
    if width==0 || height==0 || bytes>64*1024*1024 || target.len()!=bytes {
        return Err(SceneError::InvalidDimensions);
    }
    // Fail closed for ANY invalid region before touching a target pixel.
    if regions.iter().any(|r|{
        u64::from(r.x)+u64::from(r.width)>width as u64 ||
        u64::from(r.y)+u64::from(r.height)>height as u64
    }) {return Err(SceneError::InvalidDimensions);}
    for r in regions {
        if r.is_empty(){continue;}
        let shifted=surfaces.iter().map(|s|translate(s,r.x,r.y))
            .collect::<Vec<_>>();
        let patch=scene::compose(r.width as usize,r.height as usize,
                                 background,&shifted)?;
        for y in 0..r.height as usize {
            let dst=((r.y as usize+y)*width+r.x as usize)*4;
            let src=y*r.width as usize*4;
            let len=r.width as usize*4;
            target[dst..dst+len].copy_from_slice(&patch[src..src+len]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::scene::Layer;
    #[test]
    fn patches_only_changed_rectangles_and_matches_full_scene() {
        let background=[3,5,7,0];
        let rgba=[20u8,30,40,128];
        let surfaces=[
            Surface{x:-2,y:1,width:6,height:5,layer:Layer::Normal,
                pixels:Pixels::Solid([100,90,80,0])},
            Surface{x:3,y:2,width:4,height:4,layer:Layer::Overlay,
                pixels:Pixels::Solid([17,22,27,0])},
            Surface{x:8,y:7,width:1,height:1,layer:Layer::Overlay,
                pixels:Pixels::PremultipliedAlphaBgra{data:&rgba,stride:4}},
        ];
        let reference=scene::compose(12,10,background,&surfaces).unwrap();
        let mut target=vec![255u8;12*10*4];
        let rects=[DamageRect{x:0,y:0,width:7,height:7},
                   DamageRect{x:8,y:7,width:1,height:1}];
        compose_regions_into(12,10,background,&surfaces,&rects,&mut target).unwrap();
        for y in 0..10 {
            for x in 0..12 {
                let i=(y*12+x)*4;
                if x<7 && y<7 || x==8 && y==7 {
                    assert_eq!(&target[i..i+4],&reference[i..i+4]);
                }else{
                    assert_eq!(&target[i..i+4],&[255;4]);
                }
            }
        }
    }
    #[test]
    fn erased_window_repaints_background_and_unaffected_pixels_stay() {
        let mut target=vec![200u8;8*6*4];
        let r=[DamageRect{x:2,y:1,width:3,height:2}];
        compose_regions_into(8,6,[1,2,3,0],&[],&r,&mut target).unwrap();
        assert_eq!(&target[(1*8+2)*4..(1*8+3)*4],&[1,2,3,0]);
        assert_eq!(&target[0..4],&[200;4]);
    }
    #[test]
    fn invalid_region_never_partially_mutates_existing_frame() {
        let mut target=vec![4u8;8*8*4];
        let good=DamageRect{x:0,y:0,width:2,height:2};
        let invalid=DamageRect{x:7,y:7,width:2,height:2};
        assert_eq!(compose_regions_into(8,8,[0;4],&[],&[good,invalid],&mut target),
                   Err(SceneError::InvalidDimensions));
        assert!(target.iter().all(|b|*b==4));
    }
    #[test]
    fn empty_regions_are_a_noop() {
        let mut target=vec![44;4*4*4];
        compose_regions_into(4,4,[0;4],&[],&[],&mut target).unwrap();
        assert_eq!(target,vec![44;4*4*4]);
    }
}
