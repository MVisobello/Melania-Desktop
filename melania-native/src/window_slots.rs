//! Stable, bounded client slot IDs for the disposable QEMU Wayland scene.
//!
//! A slot is not a vector index that can shift when another client exits:
//! retained KMS scene entries and input hit tests keep the same ID until the
//! slot is explicitly removed. Only a freed slot may be recycled.
//! Direct GPU removal can reserve a vacant slot until its real DRM ACK.
//! This registry does not manage DRM resources or authorize native login.
const MAX_SLOTS:usize=2;
pub const MAX_DYNAMIC_SLOTS:usize=16;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SlotError { CapacityExceeded, InvalidCapacity }

#[derive(Debug)]
pub struct WindowSlots<T> {
    slots:Vec<Option<T>>,
    retiring:std::collections::BTreeSet<u64>,
    limit:usize,
}

impl<T> Default for WindowSlots<T> {
    fn default()->Self { Self { slots:Vec::new(),retiring:Default::default(),limit:MAX_SLOTS } }
}

impl<T> WindowSlots<T> {
    pub fn with_capacity(limit:usize)->Result<Self,SlotError> {
        if limit==0 || limit>MAX_DYNAMIC_SLOTS {return Err(SlotError::InvalidCapacity);}
        Ok(Self {slots:Vec::new(),retiring:Default::default(),limit})
    }
    pub fn capacity(&self)->usize {self.limit}
}

impl<T:PartialEq> WindowSlots<T> {
    /// Re-registering the same resource is idempotent. IDs are 1-based to
    /// match compositor-local MultiWindowScene keys.
    pub fn register(&mut self,surface:T)->Result<u64,SlotError> {
        if let Some(index)=self.slots.iter().position(|entry|
            entry.as_ref()==Some(&surface)) {
            return Ok(index as u64+1);
        }
        if let Some(index)=self.slots.iter().enumerate().position(|(index,entry)|
            entry.is_none() && !self.retiring.contains(&(index as u64+1))) {
            self.slots[index]=Some(surface);
            return Ok(index as u64+1);
        }
        if self.slots.len()>=self.limit {
            return Err(SlotError::CapacityExceeded);
        }
        self.slots.push(Some(surface));
        Ok(self.slots.len() as u64)
    }

    /// Look up a compositor-owned slot without allocating one during an
    /// xdg_toplevel request that may arrive before the first wl_shm commit.
    pub fn id_of(&self,surface:&T)->Option<u64> {
        self.slots.iter().position(|entry|entry.as_ref()==Some(surface))
            .map(|index|index as u64+1)
    }

    pub fn get(&self,id:u64)->Option<&T> {
        let index=id.checked_sub(1).and_then(|n|usize::try_from(n).ok())?;
        self.slots.get(index)?.as_ref()
    }

    /// Destroy only the matching surface; repeat destroys and unknown
    /// surfaces must leave the rest of the scene unchanged.
    pub fn remove(&mut self,surface:&T)->Option<u64> {
        let index=self.slots.iter().position(|entry|
            entry.as_ref()==Some(surface))?;
        self.slots[index]=None;
        Some(index as u64+1)
    }

    /// Immediately hide the destroyed owner from lookups but reserve its
    /// slot until the GPU presenter acknowledges removal. The retired role
    /// cannot receive input or callbacks, and a replacement cannot alias it.
    pub fn begin_retirement(&mut self,surface:&T)->Option<u64> {
        let id=self.remove(surface)?;
        let inserted=self.retiring.insert(id);
        debug_assert!(inserted);
        Some(id)
    }

    /// The owner must call this only for the matching verified removal
    /// transaction of the current DRM epoch. No implicit release on timeout.
    pub fn finish_retirement(&mut self,id:u64)->bool {
        self.retiring.remove(&id)
    }

    pub fn is_retiring(&self,id:u64)->bool {self.retiring.contains(&id)}
    pub fn retiring_count(&self)->usize {self.retiring.len()}

    pub fn live_count(&self)->usize {
        self.slots.iter().filter(|entry|entry.is_some()).count()
    }
}

/// Bounded FIFO of destroyed GPU role IDs awaiting their own removal job.
/// The separate WindowSlots reservation survives dequeue and only ends at ACK.
#[derive(Debug,Default)]
pub struct GpuRemovalQueue {
    waiting:std::collections::VecDeque<u64>,
}
impl GpuRemovalQueue {
    pub fn enqueue(&mut self,id:u64)->Result<(),SlotError> {
        if id==0 || id>MAX_DYNAMIC_SLOTS as u64 {return Err(SlotError::InvalidCapacity);}
        if self.waiting.contains(&id) {return Ok(());}
        if self.waiting.len()>=MAX_DYNAMIC_SLOTS {return Err(SlotError::CapacityExceeded);}
        self.waiting.push_back(id);
        Ok(())
    }
    pub fn pop(&mut self)->Option<u64> {self.waiting.pop_front()}
    pub fn is_empty(&self)->bool {self.waiting.is_empty()}
    pub fn len(&self)->usize {self.waiting.len()}
    /// Only for permanent epoch revocation, never a successful role ACK.
    pub fn revoke(&mut self) {self.waiting.clear();}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_survive_removal_and_capacity_is_bounded() {
        let mut registry=WindowSlots::default();
        assert_eq!(registry.register("window-a"),Ok(1));
        assert_eq!(registry.id_of(&"window-a"),Some(1));
        assert_eq!(registry.id_of(&"not-mapped"),None);
        assert_eq!(registry.register("window-b"),Ok(2));
        assert_eq!(registry.register("window-a"),Ok(1));
        assert_eq!(registry.register("window-c"),Err(SlotError::CapacityExceeded));
        assert_eq!(registry.live_count(),2);
        assert_eq!(registry.remove(&"window-a"),Some(1));
        assert_eq!(registry.get(1),None);
        assert_eq!(registry.get(2),Some(&"window-b"));
        assert_eq!(registry.register("window-c"),Ok(1));
        assert_eq!(registry.get(2),Some(&"window-b"));
        assert_eq!(registry.get(1),Some(&"window-c"));
    }

    #[test]
    fn stale_and_duplicate_destroy_never_affect_new_owner() {
        let mut registry=WindowSlots::default();
        assert_eq!(registry.register(10),Ok(1));
        assert_eq!(registry.remove(&10),Some(1));
        assert_eq!(registry.register(20),Ok(1));
        assert_eq!(registry.remove(&10),None);
        assert_eq!(registry.get(1),Some(&20));
        assert_eq!(registry.remove(&20),Some(1));
        assert_eq!(registry.remove(&20),None);
        assert_eq!(registry.live_count(),0);
    }

    #[test]
    fn sixteen_client_slots_recycle_only_vacant_stable_ids() {
        let mut slots=WindowSlots::with_capacity(16).unwrap();
        for id in 1..=16 {assert_eq!(slots.register(id),Ok(id as u64));}
        assert_eq!(slots.register(17),Err(SlotError::CapacityExceeded));
        assert_eq!(slots.remove(&3),Some(3));
        assert_eq!(slots.register(17),Ok(3));
        assert_eq!(slots.get(1),Some(&1));
        assert_eq!(slots.get(3),Some(&17));
        assert_eq!(slots.get(16),Some(&16));
        assert_eq!(WindowSlots::<u32>::with_capacity(0).err(),Some(SlotError::InvalidCapacity));
        assert_eq!(WindowSlots::<u32>::with_capacity(17).err(),Some(SlotError::InvalidCapacity));
    }

    #[test]
    fn gpu_slot_cannot_alias_destroyed_role_before_confirmed_removal() {
        let mut slots=WindowSlots::default();
        assert_eq!(slots.register("A"),Ok(1));
        assert_eq!(slots.register("B"),Ok(2));
        assert_eq!(slots.begin_retirement(&"A"),Some(1));
        assert_eq!(slots.get(1),None);
        assert_eq!(slots.id_of(&"A"),None);
        assert!(slots.is_retiring(1));
        assert_eq!(slots.live_count(),1);
        assert_eq!(slots.retiring_count(),1);
        assert_eq!(slots.register("C"),Err(SlotError::CapacityExceeded));
        assert_eq!(slots.begin_retirement(&"A"),None);
        assert!(!slots.finish_retirement(2));
        assert!(slots.finish_retirement(1));
        assert!(!slots.finish_retirement(1));
        assert_eq!(slots.register("C"),Ok(1));
        assert_eq!(slots.get(1),Some(&"C"));
        assert_eq!(slots.get(2),Some(&"B"));
    }
    #[test]
    fn repeated_gpu_role_replacement_stays_within_fixed_capacity() {
        let mut slots=WindowSlots::with_capacity(16).unwrap();
        for id in 1..=16 {assert_eq!(slots.register(id),Ok(id));}
        let mut owner=1;
        for next in 17..=4112 {
            assert_eq!(slots.begin_retirement(&owner),Some(1));
            assert_eq!(slots.register(next),Err(SlotError::CapacityExceeded));
            assert!(slots.finish_retirement(1));
            assert_eq!(slots.register(next),Ok(1));
            assert_eq!(slots.live_count(),16);
            assert_eq!(slots.retiring_count(),0);
            owner=next;
        }
    }
    #[test]
    fn concurrent_gpu_removals_are_serialized_without_reusing_unacked_ids() {
        let mut slots=WindowSlots::with_capacity(16).unwrap();
        let mut queue=GpuRemovalQueue::default();
        for id in 1..=16 {assert_eq!(slots.register(id),Ok(id));}
        for id in 1..=16 {
            assert_eq!(slots.begin_retirement(&id),Some(id));
            queue.enqueue(id).unwrap();
            queue.enqueue(id).unwrap(); // duplicate event is idempotent
        }
        assert_eq!(queue.len(),16);
        assert!(queue.enqueue(0).is_err());
        assert!(queue.enqueue(17).is_err());
        for id in 1..=16 {
            assert_eq!(queue.pop(),Some(id));
            assert!(slots.is_retiring(id));
            assert!(slots.finish_retirement(id));
            assert_eq!(slots.register(id+16),Ok(id));
        }
        assert!(queue.is_empty());
        assert_eq!(slots.live_count(),16);
    }
    #[test]
    fn removal_queue_revocation_does_not_release_slot_reservation() {
        let mut slots=WindowSlots::default();
        slots.register(1).unwrap();
        let id=slots.begin_retirement(&1).unwrap();
        let mut queue=GpuRemovalQueue::default();
        queue.enqueue(id).unwrap();
        queue.revoke();
        assert!(queue.is_empty());
        assert!(slots.is_retiring(id));
    }
    #[test]
    fn invalid_ids_and_unknown_surfaces_fail_closed() {
        let mut registry=WindowSlots::default();
        assert_eq!(registry.get(0),None);
        assert_eq!(registry.get(u64::MAX),None);
        assert_eq!(registry.remove(&42),None);
        assert_eq!(registry.register(42),Ok(1));
        assert_eq!(registry.get(2),None);
        assert_eq!(registry.get(1),Some(&42));
    }
}
