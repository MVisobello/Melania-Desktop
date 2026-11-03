//! Stable, bounded client slot IDs for the disposable QEMU Wayland scene.
//!
//! A slot is not a vector index that can shift when another client exits:
//! retained KMS scene entries and input hit tests keep the same ID until the
//! slot is explicitly removed. Only a freed slot may be recycled.
//! This registry does not manage DRM resources or authorize native login.
const MAX_SLOTS:usize=2;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SlotError { CapacityExceeded }

#[derive(Debug)]
pub struct WindowSlots<T> {
    slots:Vec<Option<T>>,
}

impl<T> Default for WindowSlots<T> {
    fn default()->Self { Self { slots:Vec::new() } }
}

impl<T:PartialEq> WindowSlots<T> {
    /// Re-registering the same resource is idempotent. IDs are 1-based to
    /// match compositor-local MultiWindowScene keys.
    pub fn register(&mut self,surface:T)->Result<u64,SlotError> {
        if let Some(index)=self.slots.iter().position(|entry|
            entry.as_ref()==Some(&surface)) {
            return Ok(index as u64+1);
        }
        if let Some(index)=self.slots.iter().position(Option::is_none) {
            self.slots[index]=Some(surface);
            return Ok(index as u64+1);
        }
        if self.slots.len()>=MAX_SLOTS {
            return Err(SlotError::CapacityExceeded);
        }
        self.slots.push(Some(surface));
        Ok(self.slots.len() as u64)
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

    pub fn live_count(&self)->usize {
        self.slots.iter().filter(|entry|entry.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_survive_removal_and_capacity_is_bounded() {
        let mut registry=WindowSlots::default();
        assert_eq!(registry.register("window-a"),Ok(1));
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
