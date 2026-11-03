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

#[cfg(test)]
mod tests {
    use super::*;

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
