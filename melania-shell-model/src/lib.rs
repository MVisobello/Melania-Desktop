//! Pure, platform-independent shell rules, testable without GTK or Wayland.
//! Window manipulation and file drops are different kinds of dragging.
//! The compositor is the final authority for resize, focus and DnD.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
impl Geometry {
    pub fn new(x: i32,y: i32,width: u32,height: u32)-> Self { Self{x,y,width,height} }
    pub fn right(self)-> i64 { self.x as i64 + self.width as i64 }
    pub fn bottom(self)-> i64 { self.y as i64 + self.height as i64 }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SnapZone { Left, Right, Top, TopLeft, TopRight, BottomLeft, BottomRight }

/// Edge threshold uses logical pixels, not physical pixels (HiDPI safe).
/// Corners must take priority over an edge-only match.
pub fn detect_snap(px: i32, py: i32, output: Geometry, threshold: i32) -> Option<SnapZone> {
    if threshold <= 0 || output.width < 2 || output.height < 2 { return None; }
    let t64 = threshold as i64;
    if (px as i64) < output.x as i64-t64 || (px as i64) > output.right()+t64
       || (py as i64) < output.y as i64-t64 || (py as i64) > output.bottom()+t64 { return None; }
    let l = (px as i64-output.x as i64).abs() < threshold as i64;
    let r = (px as i64-output.right()).abs() < threshold as i64;
    let t = (py as i64-output.y as i64).abs() < threshold as i64;
    let b = (py as i64-output.bottom()).abs() < threshold as i64;
    match (l,r,t,b) {
        (true,_,true,_)=>Some(SnapZone::TopLeft),
        (_,true,true,_)=>Some(SnapZone::TopRight),
        (true,_,_,true)=>Some(SnapZone::BottomLeft),
        (_,true,_,true)=>Some(SnapZone::BottomRight),
        (_,_,true,_)=>Some(SnapZone::Top),
        (true,_,_,_)=>Some(SnapZone::Left),
        (_,true,_,_)=>Some(SnapZone::Right),
        _=>None
    }
}
/// Windows occupy the usable area, leaving room for an exclusive bottom taskbar.
/// Odd dimensions are split without lost pixels or overlaps.
pub fn snap_geometry(zone: SnapZone, usable: Geometry) -> Geometry {
    let left_width = usable.width/2;
    let right_width = usable.width-left_width;
    let top_height = usable.height/2;
    let bottom_height = usable.height-top_height;
    let mid_x = usable.x.saturating_add(left_width as i32);
    let mid_y = usable.y.saturating_add(top_height as i32);
    match zone {
        SnapZone::Left=>Geometry::new(usable.x,usable.y,left_width,usable.height),
        SnapZone::Right=>Geometry::new(mid_x,usable.y,right_width,usable.height),
        SnapZone::Top=>usable,
        SnapZone::TopLeft=>Geometry::new(usable.x,usable.y,left_width,top_height),
        SnapZone::TopRight=>Geometry::new(mid_x,usable.y,right_width,top_height),
        SnapZone::BottomLeft=>Geometry::new(usable.x,mid_y,left_width,bottom_height),
        SnapZone::BottomRight=>Geometry::new(mid_x,mid_y,right_width,bottom_height),
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct WindowEntry { pub id: u64, pub title: String, pub app_id: String, pub minimized: bool, pub urgent: bool }
#[derive(Clone,Debug,Default)]
pub struct Taskbar {
    windows: BTreeMap<u64, WindowEntry>,
    order: Vec<u64>,
    focused: Option<u64>,
}
impl Taskbar {
    pub fn add(&mut self, w: WindowEntry) {
        if !self.windows.contains_key(&w.id) { self.order.push(w.id); }
        self.windows.insert(w.id,w);
        self.check_focus();
    }
    pub fn remove(&mut self,id:u64) {
        self.windows.remove(&id);
        self.order.retain(|&n|n!=id);
        if self.focused==Some(id) { self.focused=None; }
    }
    pub fn focus(&mut self,id:u64)->bool {
        if let Some(w)=self.windows.get_mut(&id) {
            w.minimized=false;
            self.focused=Some(id);
            true
        } else { false }
    }
    pub fn minimize(&mut self,id:u64)->bool {
        if let Some(w)=self.windows.get_mut(&id) {
            w.minimized=true;
            if self.focused==Some(id) {self.focused=None;}
            true
        } else { false }
    }
    pub fn reorder(&mut self,from:usize,to:usize)->bool {
        if from>=self.order.len() || to>=self.order.len() {return false;}
        let id=self.order.remove(from); self.order.insert(to,id); true
    }
    pub fn ordered(&self)->Vec<&WindowEntry> {self.order.iter().filter_map(|id|self.windows.get(id)).collect()}
    pub fn focused(&self)->Option<u64> {self.focused}
    fn check_focus(&mut self) {if self.focused.is_some_and(|id|!self.windows.contains_key(&id)){self.focused=None;}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FileDrop { OpenWith, PinLauncher, Invalid }
/// Dropping a file on an *open application* opens/imports; dropping a .desktop
/// launcher into the pinned area is a distinct operation. Never delete the source.
pub fn classify_drop(is_file:bool,is_desktop_entry:bool,onto_running_app:bool,onto_pin_area:bool)->FileDrop {
    if !is_file {return FileDrop::Invalid;}
    if onto_running_app {return FileDrop::OpenWith;}
    if onto_pin_area && is_desktop_entry {return FileDrop::PinLauncher;}
    FileDrop::Invalid
}
#[cfg(test)]
mod tests {
use super::*;
fn screen()->Geometry { Geometry::new(0,0,1921,1079) }
#[test] fn corners_win(){assert_eq!(detect_snap(0,0,screen(),14),Some(SnapZone::TopLeft));assert_eq!(detect_snap(1921,1079,screen(),14),Some(SnapZone::BottomRight));}
#[test] fn left_right(){assert_eq!(detect_snap(0,500,screen(),14),Some(SnapZone::Left));assert_eq!(detect_snap(1921,500,screen(),14),Some(SnapZone::Right));}
#[test] fn maximize_top(){assert_eq!(detect_snap(800,0,screen(),14),Some(SnapZone::Top));}
#[test] fn not_inside(){assert_eq!(detect_snap(700,500,screen(),14),None);}
#[test] fn disabled_threshold(){assert_eq!(detect_snap(0,0,screen(),0),None);}
#[test] fn outside_output_is_not_snap(){assert_eq!(detect_snap(0,9000,screen(),14),None);}
#[test] fn odd_width_is_preserved(){let a=snap_geometry(SnapZone::Left,screen());let b=snap_geometry(SnapZone::Right,screen());assert_eq!(a.width+b.width,1921);assert_eq!(a.right(),b.x as i64);}
#[test] fn odd_height_is_preserved(){let a=snap_geometry(SnapZone::TopLeft,screen());let b=snap_geometry(SnapZone::BottomLeft,screen());assert_eq!(a.height+b.height,1079);}
#[test] fn usable_area_respected(){let r=snap_geometry(SnapZone::Top,Geometry::new(12,20,1800,980));assert_eq!(r,Geometry::new(12,20,1800,980));}
fn window(n:u64)->WindowEntry{WindowEntry{id:n,title:format!("Finestra {n}"),app_id:"demo".into(),minimized:false,urgent:false}}
#[test] fn no_duplicate_windows(){let mut t=Taskbar::default();t.add(window(1));t.add(window(1));assert_eq!(t.ordered().len(),1);}
#[test] fn can_reorder(){let mut t=Taskbar::default();t.add(window(1));t.add(window(2));assert!(t.reorder(0,1));assert_eq!(t.ordered()[0].id,2);}
#[test] fn reorder_rejects_invalid(){let mut t=Taskbar::default();t.add(window(1));assert!(!t.reorder(0,5));}
#[test] fn close_clears_focus(){let mut t=Taskbar::default();t.add(window(1));t.focus(1);t.remove(1);assert_eq!(t.focused(),None);}
#[test] fn minimize_clears_focus(){let mut t=Taskbar::default();t.add(window(1));t.focus(1);t.minimize(1);assert_eq!(t.focused(),None);}
#[test] fn focus_restores(){let mut t=Taskbar::default();t.add(window(1));t.minimize(1);assert!(t.focus(1));assert!(!t.ordered()[0].minimized);}
#[test] fn unknown_focus_is_rejected(){assert!(!Taskbar::default().focus(123));}
#[test] fn file_to_app(){assert_eq!(classify_drop(true,false,true,false),FileDrop::OpenWith);}
#[test] fn desktop_to_pins(){assert_eq!(classify_drop(true,true,false,true),FileDrop::PinLauncher);}
#[test] fn arbitrary_file_cannot_be_pinned(){assert_eq!(classify_drop(true,false,false,true),FileDrop::Invalid);}
#[test] fn drag_directory_to_app_is_not_automatically_imported(){assert_eq!(classify_drop(false,false,true,false),FileDrop::Invalid);}
}


/// Monday-first month grid. No locale or system clock assumptions in this model.
/// Empty cells are kept as `None` so the renderer cannot shift weekdays.
pub fn month_grid(year: i32, month: u32) -> Vec<Option<u32>> {
    if !(1..=12).contains(&month) {return vec![];}
    let days = match month {
        2 if year%4==0 && (year%100!=0 || year%400==0) => 29,
        2 => 28,
        4|6|9|11 => 30,
        _ => 31,
    };
    // Tomohiko Sakamoto's weekday algorithm: 0=Sunday, 1=Monday.
    let offsets=[0,3,2,5,0,3,5,1,4,6,2,4];
    let adjusted=if month<3 {year-1} else {year};
    let weekday=(adjusted+adjusted/4-adjusted/100+adjusted/400+offsets[(month-1) as usize]+1)
        .rem_euclid(7);
    let monday_offset=((weekday+6)%7) as usize;
    let mut grid=vec![None;monday_offset];
    grid.extend((1..=days).map(Some));
    while grid.len()%7!=0 {grid.push(None);}
    grid
}

#[cfg(test)]
mod calendar_tests {
    use super::*;
    #[test] fn october_2026_starts_on_thursday() {
        let grid=month_grid(2026,10);
        assert_eq!(grid[3],Some(1));
        assert_eq!(grid.iter().flatten().count(),31);
    }
    #[test] fn february_leap_year_and_century() {
        assert_eq!(month_grid(2024,2).iter().flatten().count(),29);
        assert_eq!(month_grid(1900,2).iter().flatten().count(),28);
        assert_eq!(month_grid(2000,2).iter().flatten().count(),29);
    }
    #[test] fn invalid_month_does_not_panic() { assert!(month_grid(2026,13).is_empty()); }
    #[test] fn every_month_is_full_weeks() {
        for month in 1..=12 { assert_eq!(month_grid(2026,month).len()%7,0); }
    }
}
