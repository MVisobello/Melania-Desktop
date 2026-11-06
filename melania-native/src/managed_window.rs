//! Compositor-side native window state policy, independent of GTK titlebar
//! buttons and any previously approved visual styles.
use crate::window_interaction::WindowRect;
use crate::snap_layout::{snap_rect,SnapZone};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum WindowDisplayState {Normal,Maximized,Fullscreen,Minimized,Snapped(SnapZone)}
/// Super+Down follows traditional window semantics: a normal window is
/// minimized, a snapped/maximized one is restored one step, while fullscreen
/// and already hidden roles are left to the owning client.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DownShortcutAction { Minimize, Restore, Ignore }

#[derive(Debug)]
pub struct ManagedWindow {
    normal:WindowRect,
    state:WindowDisplayState,
    before_minimise:WindowDisplayState,
    output_width:i32,
    output_height:i32,
    work_area:WindowRect,
}
impl ManagedWindow {
    pub fn new(geometry:WindowRect,width:i32,height:i32)->Option<Self> {
        if width<96 || height<72 || width>16384 || height>16384 ||
           !geometry.valid_for(width,height) {return None;}
        Some(Self {
            normal:geometry,state:WindowDisplayState::Normal,
            before_minimise:WindowDisplayState::Normal,
            output_width:width,output_height:height,
            work_area:WindowRect{x:0,y:0,width,height},
        })
    }
    pub fn state(&self)->WindowDisplayState {self.state}
    pub fn down_shortcut_action(&self)->DownShortcutAction {
        match self.state {
            WindowDisplayState::Normal=>DownShortcutAction::Minimize,
            WindowDisplayState::Maximized | WindowDisplayState::Snapped(_)=>
                DownShortcutAction::Restore,
            WindowDisplayState::Fullscreen | WindowDisplayState::Minimized=>
                DownShortcutAction::Ignore,
        }
    }
    pub fn work_area(&self)->WindowRect {self.work_area}
    /// A mapped layer-shell panel may reserve an inset region of an output.
    /// Reject invalid/incomplete reservations and never use it for fullscreen.
    /// The caller must send a real XDG configure for any changed geometry.
    pub fn set_work_area(&mut self,area:WindowRect)->bool {
        if !area.valid_for(self.output_width,self.output_height) ||
           area==self.work_area {return false;}
        self.work_area=area;
        for state in [&mut self.state,&mut self.before_minimise] {
            if let WindowDisplayState::Snapped(zone)=*state {
                if snap_rect(zone,area,self.output_width,self.output_height).is_none() {
                    *state=WindowDisplayState::Normal;
                }
            }
        }
        true
    }
    pub fn geometry(&self)->Option<WindowRect> {
        match self.state {
            WindowDisplayState::Normal=>Some(self.normal),
            WindowDisplayState::Maximized=>Some(self.work_area),
            WindowDisplayState::Fullscreen=>
                Some(WindowRect{x:0,y:0,width:self.output_width,height:self.output_height}),
            WindowDisplayState::Snapped(zone)=>snap_rect(zone,
                self.work_area,self.output_width,self.output_height),
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
           snap_rect(zone,self.work_area,
                self.output_width,self.output_height).is_none() ||
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
        // A changed connector mode invalidates old panel reservations.
        // A newly mapped layer-shell surface must explicitly reapply them.
        self.work_area=WindowRect{x:0,y:0,width,height};
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
    fn live_taskbar_reservations_reflow_snap_and_maximize_but_not_fullscreen() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.snap(SnapZone::RightHalf));
        let panel=WindowRect{x:0,y:0,width:1280,height:748};
        assert!(w.set_work_area(panel));
        assert_eq!(w.geometry(),Some(WindowRect{x:640,y:0,width:640,height:748}));
        assert!(w.snap(SnapZone::RightThird));
        assert_eq!(w.geometry(),Some(WindowRect{x:853,y:0,width:427,height:748}));
        assert!(w.maximize());
        assert_eq!(w.geometry(),Some(panel));
        assert!(w.fullscreen());
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:1280,height:800}));
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(window()));
        assert_eq!(w.work_area(),panel);
        assert!(w.snap(SnapZone::BottomLeft));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:374,width:640,height:374}));
        assert!(w.set_work_area(WindowRect{x:0,y:40,width:1280,height:760}));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:420,width:640,height:380}));
        assert!(!w.set_work_area(w.work_area()));
    }
    #[test]
    fn invalid_panel_or_hotplug_rejected_and_minimized_snap_revalidated() {
        let mut w=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(w.snap(SnapZone::MiddleThird));
        assert!(w.minimize());
        let unchanged=w.work_area();
        for area in [
            WindowRect{x:0,y:0,width:0,height:800},
            WindowRect{x:0,y:740,width:1280,height:60},
            WindowRect{x:1100,y:0,width:300,height:800},
        ] {
            assert!(!w.set_work_area(area));
            assert_eq!(w.work_area(),unchanged);
        }
        assert!(w.set_work_area(WindowRect{x:0,y:0,width:250,height:140}));
        assert!(w.restore());
        assert_eq!(w.state(),WindowDisplayState::Normal);
        assert_eq!(w.geometry(),Some(window()));
        assert!(w.reconfigure_output(1920,1080));
        assert_eq!(w.work_area(),WindowRect{x:0,y:0,width:1920,height:1080});
        assert!(w.snap(SnapZone::LeftThird));
        assert_eq!(w.geometry(),Some(WindowRect{x:0,y:0,width:640,height:1080}));
    }
    #[test]
    fn real_layer_shell_reservation_feeds_snap_geometry_without_touching_fullscreen() {
        use crate::layer_surface_layout::{
            place,Anchors,Insets,LayerRequest,ShellLayer,WorkAreaReservations
        };
        let taskbar=place(LayerRequest {
            layer:ShellLayer::Top,
            anchors:Anchors{top:false,bottom:true,left:true,right:true},
            width:0,height:52,
            margins:Insets::default(),exclusive_zone:52
        },1280,800).unwrap();
        let mut reservations=WorkAreaReservations::default();
        reservations.add(taskbar.reservation);
        let area=reservations.normal_area(1280,800).unwrap();
        let mut window=ManagedWindow::new(window(),1280,800).unwrap();
        assert!(window.set_work_area(WindowRect{
            x:area.x,y:area.y,width:area.width as i32,height:area.height as i32
        }));
        assert!(window.snap(SnapZone::MiddleThird));
        assert_eq!(window.geometry(),Some(WindowRect{x:426,y:0,width:427,height:748}));
        assert!(window.maximize());
        assert_eq!(window.geometry(),Some(WindowRect{x:0,y:0,width:1280,height:748}));
        assert!(window.fullscreen());
        assert_eq!(window.geometry(),Some(WindowRect{x:0,y:0,width:1280,height:800}));
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

#[cfg(test)]
mod down_shortcut_tests {
    use super::*;
    #[test]
    fn super_down_restores_snapped_or_maximized_then_minimizes_normal() {
        let initial=WindowRect{x:110,y:90,width:420,height:300};
        let mut w=ManagedWindow::new(initial,1280,800).unwrap();
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Minimize);
        assert!(w.snap(SnapZone::LeftHalf));
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Restore);
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(initial));
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Minimize);
        assert!(w.minimize());
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Ignore);
        assert!(w.restore());
        assert_eq!(w.geometry(),Some(initial));
        assert!(w.maximize());
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Restore);
        assert!(w.restore());
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Minimize);
    }
    #[test]
    fn fullscreen_and_hidden_clients_are_not_minimized_by_super_down() {
        let initial=WindowRect{x:110,y:90,width:420,height:300};
        let mut w=ManagedWindow::new(initial,1280,800).unwrap();
        assert!(w.fullscreen());
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Ignore);
        assert!(w.restore());
        assert!(w.minimize());
        assert_eq!(w.down_shortcut_action(),DownShortcutAction::Ignore);
    }
}
