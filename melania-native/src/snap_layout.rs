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

/// Target for a completed, authenticated xdg_toplevel.move drag.
/// This is only a policy decision, not an XDG configure or KMS success.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum EdgeDrop { Snap(SnapZone), Maximize }

/// Detect a deliberate release at an output work-area boundary.
/// Bottom edge by itself is not a snap; corners take precedence over sides.
/// Reject invalid geometry, coordinates and unreasonably large hot zones.
pub fn edge_drop_target(
    x:f64,y:f64,work:WindowRect,output_w:i32,output_h:i32,tolerance:i32
)->Option<EdgeDrop> {
    if !x.is_finite() || !y.is_finite() ||
       !(1..=64).contains(&tolerance) ||
       !work.valid_for(output_w,output_h) ||
       x<f64::from(work.x) || y<f64::from(work.y) ||
       x>=f64::from(work.x+work.width) ||
       y>=f64::from(work.y+work.height) {
        return None;
    }
    let left=x<=f64::from(work.x+tolerance);
    let right=x>=f64::from(work.x+work.width-1-tolerance);
    let top=y<=f64::from(work.y+tolerance);
    let bottom=y>=f64::from(work.y+work.height-1-tolerance);
    let result=match (left,right,top,bottom) {
        (true,_,true,_)=>EdgeDrop::Snap(SnapZone::TopLeft),
        (_,true,true,_)=>EdgeDrop::Snap(SnapZone::TopRight),
        (true,_,_,true)=>EdgeDrop::Snap(SnapZone::BottomLeft),
        (_,true,_,true)=>EdgeDrop::Snap(SnapZone::BottomRight),
        (true,_,_,_)=>EdgeDrop::Snap(SnapZone::LeftHalf),
        (_,true,_,_)=>EdgeDrop::Snap(SnapZone::RightHalf),
        (_,_,true,_)=>EdgeDrop::Maximize,
        _=>return None,
    };
    Some(result)
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
    #[test]
    fn drag_edges_prioritize_corners_and_leave_bottom_alone() {
        let w=full(1280,800);
        for (point,expected) in [
            ((0.0,400.0),Some(EdgeDrop::Snap(SnapZone::LeftHalf))),
            ((1279.0,400.0),Some(EdgeDrop::Snap(SnapZone::RightHalf))),
            ((500.0,0.0),Some(EdgeDrop::Maximize)),
            ((0.0,0.0),Some(EdgeDrop::Snap(SnapZone::TopLeft))),
            ((1279.0,0.0),Some(EdgeDrop::Snap(SnapZone::TopRight))),
            ((0.0,799.0),Some(EdgeDrop::Snap(SnapZone::BottomLeft))),
            ((1279.0,799.0),Some(EdgeDrop::Snap(SnapZone::BottomRight))),
            ((500.0,799.0),None),
            ((500.0,500.0),None),
        ] {
            assert_eq!(edge_drop_target(point.0,point.1,w,1280,800,24),expected);
        }
    }
    #[test]
    fn edge_targets_obey_work_area_panel_insets_and_invalid_release() {
        let w=WindowRect{x:80,y:40,width:1040,height:680};
        assert_eq!(edge_drop_target(81.0,100.0,w,1280,800,24),
                   Some(EdgeDrop::Snap(SnapZone::LeftHalf)));
        assert_eq!(edge_drop_target(600.0,41.0,w,1280,800,24),
                   Some(EdgeDrop::Maximize));
        assert_eq!(edge_drop_target(600.0,20.0,w,1280,800,24),None);
        assert_eq!(edge_drop_target(1279.0,100.0,w,1280,800,24),None);
        assert_eq!(edge_drop_target(f64::NAN,41.0,w,1280,800,24),None);
        assert_eq!(edge_drop_target(f64::INFINITY,41.0,w,1280,800,24),None);
        assert_eq!(edge_drop_target(81.0,100.0,w,1280,800,0),None);
        assert_eq!(edge_drop_target(81.0,100.0,w,1280,800,99),None);
    }

}
