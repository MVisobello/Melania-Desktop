//! Non-destructive DRM/KMS output discovery, stage 3 of the native compositor.
//! Only read-only resource_handles/get_connector; no set_crtc, surfaces,
//! buffers, device reset or modesets.
use drm::control::{connector, Device as ControlDevice};
use smithay::backend::drm::DrmDeviceFd;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}
impl DisplayMode {
    /// Reject bogus EDID modes without relying on arbitrary defaults.
    pub fn valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.refresh_hz > 0
    }
    fn score(&self) -> (u64,u32,u32,u32) {
        (u64::from(self.width)*u64::from(self.height),
         self.refresh_hz,self.width,self.height)
    }
}

/// Provisional recommendation only: the EDID preferred bit and CRTC/plane
/// compatibility still need inspection before actual modesetting.
pub fn recommended_mode(modes:&[DisplayMode]) -> Option<DisplayMode> {
    modes.iter().filter(|mode|mode.valid())
        .max_by_key(|mode|mode.score()).cloned()
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ConnectorSnapshot {
    pub name: String,
    pub connected: bool,
    pub physical_size_mm: Option<(u32,u32)>,
    pub advertised_modes: Vec<DisplayMode>,
    pub recommendation: Option<DisplayMode>,
}
impl ConnectorSnapshot {
    /// Never infer an active output from a disconnected cable or corrupt EDID.
    pub fn viable(&self)->bool {
        self.connected && self.recommendation.is_some()
    }
}

fn to_snapshot(info:&connector::Info) -> ConnectorSnapshot {
    let connected=info.state()==connector::State::Connected;
    let name=format!("{}-{}",info.interface().as_str(),info.interface_id());
    let mut advertised_modes=if connected {
        info.modes().iter().map(|mode| {
            let (width,height)=mode.size();
            DisplayMode {width:u32::from(width),height:u32::from(height),
                refresh_hz:mode.vrefresh() as u32}
        }).filter(|mode|mode.valid()).collect::<Vec<_>>()
    } else {Vec::new()};
    // Deterministic order permits future multi-monitor reconciliation.
    advertised_modes.sort_unstable_by_key(|m|(m.width,m.height,m.refresh_hz));
    advertised_modes.dedup();
    let recommendation=recommended_mode(&advertised_modes);
    ConnectorSnapshot {name,connected,
        physical_size_mm:info.size(),
        advertised_modes,recommendation}
}

/// Read every connector from the DRM fd acquired through libseat. One
/// disappearing hotplug connector must not hide all the remaining displays.
pub fn inspect_connectors(fd:&DrmDeviceFd) -> Result<Vec<ConnectorSnapshot>,String> {
    let resources=fd.resource_handles()
        .map_err(|e|format!("DRM resource discovery failed: {e}"))?;
    let mut outputs=Vec::new();
    for handle in resources.connectors() {
        match fd.get_connector(*handle,false) {
            Ok(info)=>outputs.push(to_snapshot(&info)),
            Err(error)=>eprintln!("Melania: inaccessible DRM connector {handle:?}: {error}"),
        }
    }
    outputs.sort_by(|a,b|a.name.cmp(&b.name));
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mode(width:u32,height:u32,refresh_hz:u32)->DisplayMode {
        DisplayMode{width,height,refresh_hz}
    }
    #[test]
    fn pick_largest_resolution_then_refresh_without_mutating_modes() {
        let modes=[mode(1920,1080,60),mode(2560,1440,60),
                   mode(1920,1080,144),mode(2560,1440,75)];
        assert_eq!(recommended_mode(&modes),Some(mode(2560,1440,75)));
        assert_eq!(modes.len(),4);
    }
    #[test]
    fn zero_bogus_and_empty_modes_do_not_create_fake_outputs() {
        assert_eq!(recommended_mode(&[]),None);
        assert_eq!(recommended_mode(&[mode(0,1080,60),mode(800,0,60),mode(640,480,0)]),None);
        let unplugged=ConnectorSnapshot {
            name:"HDMI-A-1".into(),connected:false,physical_size_mm:None,
            advertised_modes:vec![mode(1920,1080,60)],
            recommendation:Some(mode(1920,1080,60)),
        };
        assert!(!unplugged.viable());
    }
    #[test]
    fn valid_connected_monitor_is_candidate_only_not_activated() {
        let connected=ConnectorSnapshot {
            name:"DP-1".into(),connected:true,
            physical_size_mm:Some((600,340)),
            advertised_modes:vec![mode(2560,1440,60)],
            recommendation:Some(mode(2560,1440,60)),
        };
        assert!(connected.viable());
        // KMS is still deliberately blocked by the native lifecycle.
        assert!(!crate::NativeLifecycle::default().can_present_kms());
    }
}


/// CRTC compatibility discovered through real DRM encoders. This is an
/// *advisory* route, not an activated scanout or a claimed plane.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OutputRoute {
    pub connector: String,
    pub possible_crtcs: Vec<String>,
}
/// Each connector is enumerated independently so a removed encoder does not
/// make every monitor disappear during hotplug.
pub fn inspect_output_routes(fd:&DrmDeviceFd) -> Result<Vec<OutputRoute>,String> {
    let resources=fd.resource_handles()
        .map_err(|e|format!("DRM CRTC discovery failed: {e}"))?;
    let mut routes=Vec::new();
    for connector_handle in resources.connectors() {
        let info=match fd.get_connector(*connector_handle,false) {
            Ok(info)=>info,
            Err(error)=>{
                eprintln!("Melania: connector vanished while matching CRTC: {error}");
                continue;
            }
        };
        if info.state()!=connector::State::Connected || info.modes().is_empty(){
            continue;
        }
        let name=format!("{}-{}",info.interface().as_str(),info.interface_id());
        let mut possible_crtcs=Vec::new();
        for handle in info.encoders() {
            let encoder=match fd.get_encoder(*handle) {
                Ok(encoder)=>encoder,
                Err(error)=>{
                    eprintln!("Melania: encoder vanished for {name}: {error}");
                    continue;
                }
            };
            for crtc in resources.filter_crtcs(encoder.possible_crtcs()) {
                possible_crtcs.push(format!("{crtc:?}"));
            }
        }
        possible_crtcs.sort();
        possible_crtcs.dedup();
        routes.push(OutputRoute{connector:name,possible_crtcs});
    }
    routes.sort_by(|a,b|a.connector.cmp(&b.connector));
    Ok(routes)
}

/// Find a maximum-cardinality one-to-one mapping of candidate connectors to
/// CRTCs, without ever touching a KMS plane. A simple greedy pass can lose
/// a usable display when two connectors share one CRTC; augmenting paths
/// relocate already assigned connectors to alternatives.
pub fn plan_crtc_routes(routes:&[OutputRoute]) -> Vec<(String,String)> {
    use std::collections::{BTreeMap,BTreeSet};
    fn assign(name:&str, by_name:&BTreeMap<String,&OutputRoute>,
              seen:&mut BTreeSet<String>, owners:&mut BTreeMap<String,String>) -> bool {
        let Some(route)=by_name.get(name) else {return false;};
        for crtc in &route.possible_crtcs {
            if !seen.insert(crtc.clone()){continue;}
            let previous=owners.get(crtc).cloned();
            if previous.is_none_or(|old|assign(&old,by_name,seen,owners)) {
                owners.insert(crtc.clone(),name.to_owned());
                return true;
            }
        }
        false
    }
    let mut by_name=BTreeMap::new();
    for route in routes {
        by_name.insert(route.connector.clone(),route);
    }
    let mut owners=BTreeMap::new();
    for name in by_name.keys(){
        let mut seen=BTreeSet::new();
        assign(name,&by_name,&mut seen,&mut owners);
    }
    let mut layout=owners.into_iter().map(|(crtc,connector)|(connector,crtc))
        .collect::<Vec<_>>();
    layout.sort();
    layout
}

#[cfg(test)]
mod route_tests {
    use super::*;
    fn r(name:&str, crtcs:&[&str])->OutputRoute {
        OutputRoute{
            connector:name.into(),
            possible_crtcs:crtcs.iter().map(|c|c.to_string()).collect()
        }
    }
    #[test]
    fn two_monitors_never_share_a_crtc() {
        let actual=plan_crtc_routes(&[r("DP-1",&["1","2"]),r("HDMI-A-1",&["1"])]);
        assert_eq!(actual.len(),2);
        assert_eq!(actual[0],("DP-1".into(),"2".into()));
        assert_eq!(actual[1],("HDMI-A-1".into(),"1".into()));
    }
    #[test]
    fn planner_does_not_invent_crtcs_for_unrouteable_monitors() {
        let actual=plan_crtc_routes(&[r("DP-1",&[]),r("DP-2",&["1"]),r("DP-3",&["1"])]);
        assert_eq!(actual.len(),1);
        assert!(actual[0].0=="DP-2" || actual[0].0=="DP-3");
        assert_eq!(actual[0].1,"1");
    }
    #[test]
    fn order_of_discovery_does_not_change_mapping() {
        let a=[r("eDP-1",&["1","3"]),r("DP-2",&["1","2"]),r("DP-1",&["3"])];
        let b=[a[2].clone(),a[0].clone(),a[1].clone()];
        assert_eq!(plan_crtc_routes(&a),plan_crtc_routes(&b));
    }
}
