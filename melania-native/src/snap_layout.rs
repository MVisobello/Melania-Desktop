//! Exact, gap-free compositor snap geometry in physical-output logical coordinates.
//! This is a policy building block; it does NOT simulate an xdg configure or KMS ACK.
use crate::window_interaction::WindowRect;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SnapZone {
    LeftHalf, RightHalf,
    TopLeft, TopRight, BottomLeft, BottomRight,
    LeftThird, MiddleThird, RightThird,
}

/// All returned rectangles are inside the supplied work area and output.
/// Odd-size divides use shared integer cut points: no overlapping columns,
/// blank pixel strips, or hard-coded 1280x800 assumptions.
/// Work area can exclude a top/bottom panel; coordinates remain output-local.
pub fn snap_rect(
    zone:SnapZone,work:WindowRect,output_width:i32,output_height:i32
)->Option<WindowRect> {
    if output_width>16384 || output_height>16384 ||
       !work.valid_for(output_width,output_height) {
        return None;
    }
    let (left,right)=match zone {
        SnapZone::LeftHalf|SnapZone::TopLeft|SnapZone::BottomLeft=>
            (0,work.width/2),
        SnapZone::RightHalf|SnapZone::TopRight|SnapZone::BottomRight=>
            (work.width/2,work.width),
        SnapZone::LeftThird=>(0,work.width/3),
        SnapZone::MiddleThird=>(work.width/3,work.width*2/3),
        SnapZone::RightThird=>(work.width*2/3,work.width),
    };
    let (top,bottom)=match zone {
        SnapZone::TopLeft|SnapZone::TopRight=>(0,work.height/2),
        SnapZone::BottomLeft|SnapZone::BottomRight=>(work.height/2,work.height),
        _=>(0,work.height),
    };
    let rect=WindowRect {
        x:work.x.checked_add(left)?,
        y:work.y.checked_add(top)?,
        width:right-left,
        height:bottom-top,
    };
    rect.valid_for(output_width,output_height).then_some(rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn full(w:i32,h:i32)->WindowRect {
        WindowRect{x:0,y:0,width:w,height:h}
    }
    #[test]
    fn two_halves_and_four_corners_are_gap_free_on_odd_output() {
        let work=full(1919,1079);
        let l=snap_rect(SnapZone::LeftHalf,work,1919,1079).unwrap();
        let r=snap_rect(SnapZone::RightHalf,work,1919,1079).unwrap();
        assert_eq!(l,WindowRect{x:0,y:0,width:959,height:1079});
        assert_eq!(r,WindowRect{x:959,y:0,width:960,height:1079});
        let tl=snap_rect(SnapZone::TopLeft,work,1919,1079).unwrap();
        let tr=snap_rect(SnapZone::TopRight,work,1919,1079).unwrap();
        let bl=snap_rect(SnapZone::BottomLeft,work,1919,1079).unwrap();
        let br=snap_rect(SnapZone::BottomRight,work,1919,1079).unwrap();
        assert_eq!(tl,WindowRect{x:0,y:0,width:959,height:539});
        assert_eq!(tr.x,tl.width);
        assert_eq!(bl.y,tl.height);
        assert_eq!(br.x,tr.x);
        assert_eq!(br.y,bl.y);
        assert_eq!(tl.width*tl.height+tr.width*tr.height+
                   bl.width*bl.height+br.width*br.height,1919*1079);
    }
    #[test]
    fn thirds_cover_output_without_seams() {
        let work=full(1366,768);
        let a=snap_rect(SnapZone::LeftThird,work,1366,768).unwrap();
        let b=snap_rect(SnapZone::MiddleThird,work,1366,768).unwrap();
        let c=snap_rect(SnapZone::RightThird,work,1366,768).unwrap();
        assert_eq!((a.width,b.width,c.width),(455,455,456));
        assert_eq!((a.x,b.x,c.x),(0,455,910));
        assert_eq!(a.width+b.width+c.width,1366);
    }
    #[test]
    fn reserved_panel_and_offset_work_area_are_respected() {
        let work=WindowRect{x:160,y:36,width:960,height:700};
        let a=snap_rect(SnapZone::TopLeft,work,1280,800).unwrap();
        let b=snap_rect(SnapZone::BottomRight,work,1280,800).unwrap();
        assert_eq!(a,WindowRect{x:160,y:36,width:480,height:350});
        assert_eq!(b,WindowRect{x:640,y:386,width:480,height:350});
        assert!(a.valid_for(1280,800) && b.valid_for(1280,800));
    }
    #[test]
    fn rejects_too_small_zones_bad_work_areas_and_overflow() {
        assert_eq!(snap_rect(SnapZone::LeftThird,full(240,150),240,150),None);
        assert_eq!(snap_rect(SnapZone::TopLeft,full(640,120),640,120),None);
        assert_eq!(snap_rect(SnapZone::LeftHalf,full(0,800),1280,800),None);
        assert_eq!(snap_rect(SnapZone::RightHalf,
                  WindowRect{x:1000,y:0,width:500,height:800},1280,800),None);
        assert_eq!(snap_rect(SnapZone::LeftHalf,full(16385,800),16385,800),None);
        assert_eq!(snap_rect(SnapZone::RightHalf,
                  WindowRect{x:i32::MAX,y:0,width:640,height:800},1280,800),None);
    }
}
