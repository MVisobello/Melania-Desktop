//! Bounded, per-surface buffer receipts for native continuous KMS.
//!
//! A single "latest client" receipt loses independent Wayland surfaces when
//! both commit while a framebuffer is outstanding. This ledger retains at
//! most one copied-buffer receipt per surface for the next composition.
//! The compositor owns independent pixels in MultiWindowScene; superseded
//! wl_buffer handles can be released immediately after copying.
//!
//! A real KMS ACK authorizes a surface frame callback only when no newer
//! wl_shm commit from THAT surface is waiting. Updates on another surface
//! must not starve the previously displayed client.
//!
//! Backend-neutral: no DRM device, Wayland handle or IO.
pub const MAX_PENDING_CLIENTS: usize = 2;
pub const MAX_DYNAMIC_PENDING_CLIENTS: usize = 16;

#[derive(Debug)]
pub struct ClientCommitLedger<K, V> {
    waiting: Vec<(K, V)>,
    capacity:usize,
}

impl<K,V> Default for ClientCommitLedger<K,V> {
    fn default()->Self {Self {waiting:Vec::new(),capacity:MAX_PENDING_CLIENTS}}
}

#[derive(Debug)]
pub struct CompletedClient<K, V> {
    pub key: K,
    pub resource: V,
    pub notify_frame: bool,
}

impl<K: PartialEq, V> ClientCommitLedger<K, V> {
    pub fn new() -> Self { Self::default() }
    pub fn with_capacity(capacity:usize)->Option<Self> {
        if capacity==0 || capacity>MAX_DYNAMIC_PENDING_CLIENTS {return None;}
        Some(Self {waiting:Vec::new(),capacity})
    }
    pub fn capacity(&self)->usize {self.capacity}
    pub fn waiting_len(&self) -> usize { self.waiting.len() }

    /// The caller releases the returned old buffer: its pixels are already
    /// copied into compositor-owned memory. On capacity error the caller
    /// retains ownership of the rejected new resource.
    pub fn stage(&mut self, key: K, resource: V) -> Result<Option<V>, (K, V)> {
        if let Some(index) = self.waiting.iter()
            .position(|(stored, _)| stored == &key) {
            return Ok(Some(std::mem::replace(&mut self.waiting[index].1, resource)));
        }
        if self.waiting.len() >= self.capacity {
            return Err((key, resource));
        }
        self.waiting.push((key, resource));
        Ok(None)
    }

    /// Called after the compositor successfully queues the corresponding
    /// full-scene pixels for DRM/KMS; newer commits stay in waiting.
    pub fn take_for_presentation(&mut self) -> Vec<(K, V)> {
        std::mem::take(&mut self.waiting)
    }

    /// Use only after the external DRM presenter has verified actual CRTC
    /// scanout. Newer damage from the same surface delays its frame callback.
    pub fn acknowledged(&self, submitted: Vec<(K, V)>) -> Vec<CompletedClient<K, V>> {
        submitted.into_iter().map(|(key, resource)| {
            let notify_frame = !self.waiting.iter().any(|(newer, _)| newer == &key);
            CompletedClient { key, resource, notify_frame }
        }).collect()
    }
}

/// A direct GPU update owns an unconsumed client buffer, unlike the
/// CPU ledger whose pixels are already copied. Keep one active sampling
/// transaction and at most one waiting update per surface. No receipt for
/// another surface can retire the active update.
#[derive(Debug)]
pub struct GpuClientQueue<K,V> {
    waiting:std::collections::VecDeque<(K,V)>,
    active:Option<(K,V)>,
}
impl<K,V> Default for GpuClientQueue<K,V> {
    fn default()->Self {Self{waiting:Default::default(),active:None}}
}
#[derive(Debug,PartialEq,Eq)]
pub enum GpuQueueAdmission<V> {Queued(Option<V>),Busy,Full(V)}
impl<K:PartialEq,V:PartialEq> GpuClientQueue<K,V> {
    // One active source + up to three waiting sources, within the existing
    // four-import Wayland registry budget. Never evict an active source.
    pub fn contains_resource(&self,resource:&V)->bool {
        self.active.as_ref().is_some_and(|(_,v)|v==resource) ||
        self.waiting.iter().any(|(_,v)|v==resource)
    }
    pub fn has_waiting_for(&self,key:&K)->bool {
        self.waiting.iter().any(|(k,_)|k==key)
    }
    pub fn stage(&mut self,key:K,resource:V)->GpuQueueAdmission<V> {
        if self.contains_resource(&resource) {return GpuQueueAdmission::Busy;}
        if let Some((_,old))=self.waiting.iter_mut().find(|(k,_)|k==&key) {
            return GpuQueueAdmission::Queued(Some(std::mem::replace(old,resource)));
        }
        if self.waiting.len()>=3 {return GpuQueueAdmission::Full(resource);}
        self.waiting.push_back((key,resource));
        GpuQueueAdmission::Queued(None)
    }
    pub fn activate_next(&mut self)->Option<(K,V)> where K:Clone,V:Clone {
        if self.active.is_some(){return None;}
        let next=self.waiting.pop_front()?;
        self.active=Some(next.clone());
        Some(next)
    }
    /// Call strictly after the matching current-epoch DRM receipt.
    pub fn finish(&mut self,key:&K,resource:&V)->bool {
        if !self.active.as_ref().is_some_and(|(k,v)|k==key && v==resource) {return false;}
        self.active=None;
        true
    }
    pub fn cancel_waiting(&mut self,key:&K)->Option<V> {
        let index=self.waiting.iter().position(|(k,_)|k==key)?;
        self.waiting.remove(index).map(|(_,v)|v)
    }
    pub fn cancel_destroyed_resource(&mut self,resource:&V) {
        self.waiting.retain(|(_,v)|v!=resource);
    }
    /// Epoch revocation is not a successful presentation or release.
    pub fn revoke(&mut self){self.waiting.clear(); self.active=None;}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_queue_never_releases_active_or_other_surface_receipt() {
        let mut q=GpuClientQueue::default();
        assert_eq!(q.stage("A",1),GpuQueueAdmission::Queued(None));
        assert_eq!(q.activate_next(),Some(("A",1)));
        assert_eq!(q.stage("B",2),GpuQueueAdmission::Queued(None));
        assert_eq!(q.stage("A",3),GpuQueueAdmission::Queued(None));
        assert_eq!(q.stage("B",4),GpuQueueAdmission::Queued(Some(2)));
        assert_eq!(q.stage("C",1),GpuQueueAdmission::Busy);
        assert_eq!(q.stage("C",5),GpuQueueAdmission::Queued(None));
        assert_eq!(q.stage("D",6),GpuQueueAdmission::Full(6));
        assert!(q.has_waiting_for(&"A"));
        assert_eq!(q.activate_next(),None);
        assert!(!q.finish(&"B",&1));
        assert!(!q.finish(&"A",&3));
        assert!(q.finish(&"A",&1));
        assert_eq!(q.activate_next(),Some(("B",4)));
        assert_eq!(q.cancel_waiting(&"A"),Some(3));
        assert!(q.finish(&"B",&4));
        assert_eq!(q.activate_next(),Some(("C",5)));
        assert!(q.finish(&"C",&5));
        assert_eq!(q.activate_next(),None);
    }
    #[test]
    fn gpu_queue_repeated_buffer_reuse_requires_completion() {
        let mut q=GpuClientQueue::default();
        for _ in 0..4096 {
            assert_eq!(q.stage(1,9),GpuQueueAdmission::Queued(None));
            assert_eq!(q.activate_next(),Some((1,9)));
            assert_eq!(q.stage(1,9),GpuQueueAdmission::Busy);
            assert_eq!(q.cancel_waiting(&1),None);
            assert!(q.finish(&1,&9));
        }
        q.stage(2,8);
        q.cancel_destroyed_resource(&8);
        assert_eq!(q.activate_next(),None);
        q.stage(2,8);
        q.activate_next();
        q.revoke();
        assert!(!q.finish(&2,&8));
    }
    #[test]
    fn two_clients_survive_overlapping_commits() {
        let mut ledger=ClientCommitLedger::new();
        assert_eq!(ledger.stage("A",1),Ok(None));
        assert_eq!(ledger.stage("B",2),Ok(None));
        assert_eq!(ledger.waiting_len(),2);
        let first=ledger.take_for_presentation();
        assert_eq!(ledger.waiting_len(),0);
        // A commits again before A1/B1 is ACKed. B must be notified now;
        // A's newer callback is only valid after the next real frame.
        assert_eq!(ledger.stage("A",3),Ok(None));
        let done=ledger.acknowledged(first);
        assert_eq!(done.len(),2);
        assert_eq!((done[0].key,done[0].resource,done[0].notify_frame),("A",1,false));
        assert_eq!((done[1].key,done[1].resource,done[1].notify_frame),("B",2,true));
        let later_frame=ledger.take_for_presentation();
        let later=ledger.acknowledged(later_frame);
        assert_eq!(later.len(),1);
        assert_eq!((later[0].key,later[0].resource,later[0].notify_frame),("A",3,true));
    }

    #[test]
    fn duplicate_surface_is_bounded_and_old_buffer_returned() {
        let mut ledger=ClientCommitLedger::new();
        assert_eq!(ledger.stage(1,vec![10]),Ok(None));
        assert_eq!(ledger.stage(1,vec![11]),Ok(Some(vec![10])));
        assert_eq!(ledger.waiting_len(),1);
        assert_eq!(ledger.stage(2,vec![20]),Ok(None));
        assert_eq!(ledger.stage(3,vec![30]),Err((3,vec![30])));
        assert_eq!(ledger.waiting_len(),2);
        assert_eq!(ledger.take_for_presentation().len(),2);
        assert_eq!(ledger.waiting_len(),0);
    }

    #[test]
    fn sixteen_independent_wayland_clients_survive_one_kms_frame() {
        let mut ledger=ClientCommitLedger::with_capacity(16).unwrap();
        for id in 1..=16 {assert_eq!(ledger.stage(id,id*10),Ok(None));}
        assert_eq!(ledger.stage(17,170),Err((17,170)));
        let previous=ledger.take_for_presentation();
        assert_eq!(previous.len(),16);
        ledger.stage(1,11).unwrap();
        let acknowledged=ledger.acknowledged(previous);
        assert_eq!(acknowledged.len(),16);
        assert!(!acknowledged[0].notify_frame);
        assert!(acknowledged[1..].iter().all(|c|c.notify_frame));
        assert!(ClientCommitLedger::<u32,u32>::with_capacity(0).is_none());
        assert!(ClientCommitLedger::<u32,u32>::with_capacity(17).is_none());
    }

    #[test]
    fn other_surface_updates_do_not_starve_old_client() {
        let mut ledger=ClientCommitLedger::new();
        ledger.stage("A",1).unwrap();
        let sent=ledger.take_for_presentation();
        ledger.stage("B",2).unwrap();
        let done=ledger.acknowledged(sent);
        assert_eq!(done.len(),1);
        assert!(done[0].notify_frame);
        assert_eq!(done[0].key,"A");
        assert_eq!(ledger.waiting_len(),1);
    }
}
