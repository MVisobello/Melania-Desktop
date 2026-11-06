//! Compositor-side native window state policy, independent of GTK titlebar
//! buttons and any previously approved visual styles.
use crate::window_interaction::WindowRect;
use crate::snap_layout::{snap_rect,SnapZone};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum WindowDisplayState {Normal,Maximized,Fullscreen,Minimized,Snapped(SnapZone)}
#[derive(Debug)]
pub struct ManagedWindow {
    normal:WindowRect,
    state:WindowDisplayState,
    before_minimise:WindowDisplayState,
    output_width:i32,
    output_height:i32,
}
impl ManagedWindow {
    pub fn new(geometry:WindowRect,width:i32,height:i32)->Option<Self> {
        if width<96 || height<72 || width>16384 || height>16384 ||
           !geometry.valid_for(width,height) {return None;}
        Some(Self {
            normal:geometry,state:WindowDisplayState::Normal,
            before_minimise:WindowDisplayState::Normal,
            output_width:width,output_height:height,
        })
    }
    pub fn state(&self)->WindowDisplayState {self.state}
    pub fn geometry(&self)->Option<WindowRect> {
        match self.state {
            WindowDisplayState::Normal=>Some(self.normal),
            WindowDisplayState::Maximized | WindowDisplayState::Fullscreen=>
                Some(WindowRect{x:0,y:0,width:self.output_width,height:self.output_height}),
            WindowDisplayState::Snapped(zone)=>snap_rect(zone,
                WindowRect{x:0,y:0,width:self.output_width,height:self.output_height},
                self.output_width,self.output_height),
            WindowDisplayState::Minimized=>None,
        }
    }
    /// Synchronize the remembered restore rectangle with a real, bounded
    /// client commit or compositor move. Never overwrite it while maximized.
    pub fn update_normal_geometry(&mut self,rect:WindowRect)->bool {
        if self.state!=WindowDisplayState::Normal ||
           !rect.valid_for(self.output_width,self.output_height) || self.normal==rect {
            return false;
        }
        self.normal=rect;
        true
    }
    /// Initiate an exact snap; normal restore geometry is kept unchanged.
    /// The Wayland bridge must request a matching client configure and wait
    /// for its real wl_shm commit before repainting the actual KMS scene.
    pub fn snap(&mut self,zone:SnapZone)->bool {
        if self.state==WindowDisplayState::Minimized ||
           snap_rect(zone,WindowRect{x:0,y:0,width:self.output_width,
                height:self.output_height},self.output_width,self.output_height).is_none() ||
           self.state==WindowDisplayState::Snapped(zone) {return false;}
        self.state=WindowDisplayState::Snapped(zone);
        true
    }
    pub fn maximize(&mut self)->bool {
        if self.state==WindowDisplayState::Maximized {return false;}
        self.state=WindowDisplayState::Maximized;true
    }
    pub fn fullscreen(&mut self)->bool {
        if self.state==WindowDisplayState::Fullscreen{return false;}
        self.state=WindowDisplayState::Fullscreen;true
    }
    pub fn minimize(&mut self)->bool {
        if self.state==WindowDisplayState::Minimized{return false;}
        self.before_minimise=self.state;
        self.state=WindowDisplayState::Minimized;
        true
    }
    /// From minimized return to previously visible state; from maximized
    /// or fullscreen restore the original bounded normal geometry.
    pub fn restore(&mut self)->bool {
        if self.state==WindowDisplayState::Normal {return false;}
        self.state=if self.state==WindowDisplayState::Minimized {
            self.before_minimise
        }else{WindowDisplayState::Normal};
        true
    }
    pub fn reconfigure_output(&mut self,width:i32,height:i32)->bool {
        if width<96 || height<72 || width>16384 || height>16384{return false;}
        self.output_width=width;
        self.output_height=height;
        self.normal.width=self.normal.width.min(width);
        self.normal.height=self.normal.height.min(height);
        self.normal.x=self.normal.x.clamp(0,width-self.normal.width);
        self.normal.y=self.normal.y.clamp(0,height-self.normal.height);
        // Hotplug or resolution change must never retain an impossible tile.
        let area=WindowRect{x:0,y:0,width,height};
        if let WindowDisplayState::Snapped(zone)=self.state {
            if snap_rect(zone,area,width,height).is_none() {
                self.state=WindowDisplayState::Normal;
            }
        }
        if let WindowDisplayState::Snapped(zone)=self.before_minimise {
            if snap_rect(zone,area,width,height).is_none() {
                self.before_minimise=WindowDisplayState::Normal;
            }
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn window()->WindowRect {WindowRect{x:100,y:80,width:420,height:300}}
    #[test]
    fn maximise_minimise_restore_retains_original_geometry() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.maximize());
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:1280,height:800}));
        assert!(w.minimize());
        assert_eq!(w.geometry(),None);
        assert!(w.restore());
        assert_eq!(w.state(),WindowDisplayState::Maximized);
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(window()));
        assert!(!w.restore());
    }
    #[test]
    fn native_client_commit_updates_restore_only_in_normal_state() {
        let mut window=ManagedWindow::new(window(),1280,800).unwrap();
        let updated=WindowRect{x:240,y:180,width:320,height:192};
        assert!(window.update_normal_geometry(updated));
        assert!(!window.update_normal_geometry(updated));
        assert!(window.maximize());
        assert!(!window.update_normal_geometry(WindowRect{x:0,y:0,width:1280,height:800}));
        assert!(window.restore());
        assert_eq!(window.geometry(),Some(updated));
        assert!(!window.update_normal_geometry(WindowRect{x:-1,..updated}));
    }
    #[test]
    fn fullscreen_after_maximize_restores_normal_client_geometry() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        let moved=WindowRect{x:240,y:160,width:360,height:220};
        assert!(w.update_normal_geometry(moved));
        assert!(w.maximize());
        assert!(w.fullscreen());
        assert_eq!(w.state(),WindowDisplayState::Fullscreen);
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:1280,height:800}));
        assert!(!w.fullscreen());
        assert!(!w.update_normal_geometry(WindowRect{x:0,y:0,width:1280,height:800}));
        assert!(w.restore());
        assert_eq!(w.state(),WindowDisplayState::Normal);
        assert_eq!(w.geometry(),Some(moved));
    }
    #[test]
    fn fullscreen_and_mode_change_clip_restore_position() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.fullscreen());
        assert!(w.reconfigure_output(640,480));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:640,height:480}));
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(window()));
        assert!(w.reconfigure_output(320,200));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:320,height:200}));
        assert!(!w.reconfigure_output(0,200));
        assert!(ManagedWindow::new(window(),100,100).is_none());
    }
    #[test]
    fn snap_then_restore_preserves_unsnapped_client_geometry() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.snap(SnapZone::LeftHalf));
        assert_eq!(w.state(),WindowDisplayState::Snapped(SnapZone::LeftHalf));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:640,height:800}));
        assert!(!w.update_normal_geometry(WindowRect{x:0,y:0,width:640,height:800}));
        assert!(!w.snap(SnapZone::LeftHalf));
        assert!(w.snap(SnapZone::RightHalf));
        assert_eq!(w.geometry(),Some(WindowRect{x:640,y:0,width:640,height:800}));
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(window()));
    }
    #[test]
    fn minimize_restores_snapped_state_and_reflows_on_output_change() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.snap(SnapZone::TopRight));
        assert!(w.minimize());
        assert_eq!(w.geometry(),None);
        assert!(w.reconfigure_output(1920,1080));
        assert!(w.restore());
        assert_eq!(w.state(),WindowDisplayState::Snapped(SnapZone::TopRight));
        assert_eq!(w.geometry(),Some(WindowRect{x:960,y:0,width:960,height:540}));
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(window()));
    }
    #[test]
    fn tiny_output_drops_invalid_snap_without_changing_restore_rectangle() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.snap(SnapZone::MiddleThird));
        assert!(w.reconfigure_output(250,140));
        assert_eq!(w.state(),WindowDisplayState::Normal);
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:250,height:140}));
        assert!(!w.snap(SnapZone::MiddleThird));
    }

}
