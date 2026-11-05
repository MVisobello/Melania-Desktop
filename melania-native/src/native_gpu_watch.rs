//! DRM GPU topology watch bound to the actual Smithay/udev seat.
//!
//! A compositor which keeps drawing after a DRM card disappears or changes
//! cannot safely trust its old GBM surfaces, page-flip ACKs, or framebuffer
//! IDs. This deliberately does NOT reacquire DRM: the current QEMU bridge
//! must stop, finish its KMS rollback, and close libseat-owned descriptors.
//! A future production backend will use a fresh acquisition epoch instead.
//!
//! This is original Melania code informed by the backend separation used by
//! mature Smithay compositors; it does not contain cosmic-comp source.
use std::{collections::BTreeMap,path::PathBuf};
use crate::DeviceChange;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum GpuRevocationReason {
    Added,
    Changed,
    Removed,
    Replaced,
    BaselineConflict,
}

#[derive(Debug,Default)]
pub struct NativeGpuWatch {
    cards:BTreeMap<u64,PathBuf>,
    reason:Option<GpuRevocationReason>,
}
impl NativeGpuWatch {
    /// Snapshot the real devices observed before the bridge opens the KMS
    /// presenter. Identical repeated enumerations are harmless. A duplicate
    /// device ID with a different path makes the baseline untrustworthy.
    pub fn from_discovered(devices:impl IntoIterator<Item=(u64,PathBuf)>)->Self {
        let mut watch=Self::default();
        for (id,path) in devices {
            if let Some(old)=watch.cards.insert(id,path.clone()) {
                if old!=path {
                    watch.reason=Some(GpuRevocationReason::BaselineConflict);
                }
            }
        }
        watch
    }
    pub fn known_cards(&self)->usize {self.cards.len()}
    pub fn revoked(&self)->bool {self.reason.is_some()}
    pub fn reason(&self)->Option<GpuRevocationReason> {self.reason}

    /// Must be invoked from the SAME calloop dispatch as libseat and the
    /// Wayland compositor, before processing another frame or client ACK.
    /// Once revoked, no later Added event can restore this epoch.
    pub fn observe(&mut self,event:&DeviceChange)->Option<GpuRevocationReason> {
        if self.revoked(){return None;}
        let reason=match event {
            DeviceChange::Added{id,path}=>{
                match self.cards.get(id){
                    Some(old) if old==path=>None,
                    Some(_)=>Some(GpuRevocationReason::Replaced),
                    None=>Some(GpuRevocationReason::Added),
                }
            }
            DeviceChange::Changed{..}=>Some(GpuRevocationReason::Changed),
            DeviceChange::Removed{..}=>Some(GpuRevocationReason::Removed),
        };
        if let Some(reason)=reason {
            self.reason=Some(reason);
            Some(reason)
        } else {None}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn baseline()->NativeGpuWatch{
        NativeGpuWatch::from_discovered([(11,"/dev/dri/card0".into())])
    }
    #[test]
    fn repeated_initial_gpu_inventory_is_not_a_false_hotplug(){
        let mut watch=NativeGpuWatch::from_discovered([
            (11,"/dev/dri/card0".into()),
            (11,"/dev/dri/card0".into()),
        ]);
        assert_eq!(watch.known_cards(),1);
        assert_eq!(watch.observe(&DeviceChange::Added{
            id:11,path:"/dev/dri/card0".into()}),None);
        assert!(!watch.revoked());
    }
    #[test]
    fn any_changed_or_removed_gpu_poisoned_epoch_is_irreversible(){
        for event in [
            DeviceChange::Changed{id:11},
            DeviceChange::Removed{id:11},
            DeviceChange::Changed{id:99},
        ]{
            let mut watch=baseline();
            assert!(watch.observe(&event).is_some());
            assert!(watch.revoked());
            assert_eq!(watch.observe(&DeviceChange::Added{
                id:11,path:"/dev/dri/card0".into()}),None);
            assert!(watch.revoked());
        }
    }
    #[test]
    fn new_or_replaced_gpu_never_uses_stale_crtc_baseline(){
        let mut watch=baseline();
        assert_eq!(watch.observe(&DeviceChange::Added{
            id:12,path:"/dev/dri/card1".into()}),Some(GpuRevocationReason::Added));
        let mut watch=baseline();
        assert_eq!(watch.observe(&DeviceChange::Added{
            id:11,path:"/dev/dri/card9".into()}),Some(GpuRevocationReason::Replaced));
        assert_eq!(watch.known_cards(),1);
    }
    #[test]
    fn conflicting_device_ids_revoke_before_first_kms_frame(){
        let watch=NativeGpuWatch::from_discovered([
            (2,"/dev/dri/card0".into()),
            (2,"/dev/dri/card9".into()),
        ]);
        assert_eq!(watch.reason(),Some(GpuRevocationReason::BaselineConflict));
        assert!(watch.revoked());
    }
    #[test]
    fn empty_baseline_is_allowed_for_protocol_only_mode_but_not_a_later_gpu(){
        let mut watch=NativeGpuWatch::default();
        assert!(!watch.revoked());
        assert_eq!(watch.observe(&DeviceChange::Added{
            id:3,path:"/dev/dri/card0".into()}),Some(GpuRevocationReason::Added));
    }
}
