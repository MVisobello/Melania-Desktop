//! Monotonic kernel flip receipts for an actual DRM/KMS output.
//!
//! A successful page_flip ioctl is only a QUEUE receipt, not a presentation.
//! The compositor may complete an outstanding frame only after the matching
//! CRTC's kernel PageFlip event passes this ledger and a separate kernel
//! framebuffer+connector verification succeeds. This module never opens a
//! device or pretends that an event by itself proves scanout.
//!
//! The state contains no frame-count limit or QEMU ordinals. One instance is
//! needed per owned output/CRTC and per libseat acquisition epoch.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipLedgerError {
    Revoked,
    AlreadyPending,
    UnexpectedCompletion,
    StaleOrDuplicateVblank,
}

#[derive(Debug, Default)]
pub struct NativeFlipLedger {
    pending: bool,
    revoked: bool,
    last_vblank: Option<u32>,
    // Some virtual DRM devices do not publish a stable incrementing vblank
    // counter. Never relax this check for ordinary hardware.
    unreliable_vm_counter: bool,
    completed: u64,
}

impl NativeFlipLedger {
    pub fn has_pending(&self) -> bool { self.pending }
    pub fn completed_count(&self) -> u64 { self.completed }
    pub fn revoked(&self) -> bool { self.revoked }
    pub fn unreliable_vm_counter(&self) -> bool { self.unreliable_vm_counter }

    /// The output backend must first own a live DRM card and submit exactly
    /// one flip. This reservation prevents a concurrent second request.
    pub fn begin_flip(&mut self) -> Result<(), FlipLedgerError> {
        if self.revoked { return Err(FlipLedgerError::Revoked); }
        if self.pending { return Err(FlipLedgerError::AlreadyPending); }
        self.pending=true;
        Ok(())
    }

    /// Call only for an actual PageFlip event from the expected CRTC.
    /// Vblank frame counters are 32-bit and may wrap. A forward distance in
    /// 1..2^31 is accepted, avoiding a naïve check that fails at wraparound;
    /// zero and backwards/implausibly large jumps are rejected. Gaps are
    /// allowed (idle displays need not emit an event for every vblank).
    pub fn complete_flip(&mut self, frame: u32) -> Result<(), FlipLedgerError> {
        if self.revoked { return Err(FlipLedgerError::Revoked); }
        if !self.pending {
            // An unsolicited kernel completion makes this device epoch
            // ambiguous: never let a later flip consume a stale receipt.
            self.revoke();
            return Err(FlipLedgerError::UnexpectedCompletion);
        }
        if self.last_vblank.is_some_and(|last| {
            let distance=frame.wrapping_sub(last);
            distance==0 || distance >= (1u32 << 31)
        }) {
            self.revoke();
            return Err(FlipLedgerError::StaleOrDuplicateVblank);
        }
        let Some(completed)=self.completed.checked_add(1) else {
            self.revoke();
            return Err(FlipLedgerError::Revoked);
        };
        self.last_vblank=Some(frame);
        self.completed=completed;
        self.pending=false;
        Ok(())
    }

    /// A dedicated VirtIO/QEMU-only exception for kernel drivers that deliver
    /// a real PageFlip event but reuse or reset the reported VBlank counter.
    /// This is NOT an ACK and MUST be followed by actual kernel FB_ID and
    /// connector routing verification before returning a frame to Wayland.
    ///
    /// The caller is responsible for the QEMU DMI, dedicated VT, libseat,
    /// explicit opt-in and matching-CRTC event-reader guards. This never runs
    /// in a production login compositor or on a host physical GPU.
    pub fn complete_vm_flip(&mut self, frame: u32) -> Result<(), FlipLedgerError> {
        if self.revoked { return Err(FlipLedgerError::Revoked); }
        if !self.pending { return Err(FlipLedgerError::UnexpectedCompletion); }
        let counter_moved_forward = self.last_vblank.is_none_or(|last| {
            let distance=frame.wrapping_sub(last);
            distance!=0 && distance < (1u32 << 31)
        });
        if !counter_moved_forward {
            self.unreliable_vm_counter=true;
        }
        // A previously unreliable virtual counter cannot be treated as an
        // authoritative sequence again within the same device epoch.
        if self.unreliable_vm_counter {
            let Some(completed)=self.completed.checked_add(1) else {
                self.revoke();
                return Err(FlipLedgerError::Revoked);
            };
            self.completed=completed;
            self.last_vblank=Some(frame);
            self.pending=false;
            return Ok(());
        }
        self.complete_flip(frame)
    }

    /// A VT pause, GPU hotplug, failed flip or disputed kernel receipt must
    /// create a FRESH ledger with a new device epoch after all resources are
    /// re-acquired. Never re-arm a ledger that has seen an unsafe event.
    pub fn revoke(&mut self) {
        self.revoked=true;
        self.pending=false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_completion_without_an_outstanding_kernel_flip() {
        let mut ledger=NativeFlipLedger::default();
        assert_eq!(ledger.complete_flip(1),Err(FlipLedgerError::UnexpectedCompletion));
        assert!(ledger.revoked());
        let mut ledger=NativeFlipLedger::default();
        ledger.begin_flip().unwrap();
        assert_eq!(ledger.begin_flip(),Err(FlipLedgerError::AlreadyPending));
        ledger.complete_flip(17).unwrap();
        assert_eq!(ledger.completed_count(),1);
        assert!(!ledger.has_pending());
        assert_eq!(ledger.complete_flip(18),Err(FlipLedgerError::UnexpectedCompletion));
        assert!(ledger.revoked());
        assert_eq!(ledger.begin_flip(),Err(FlipLedgerError::Revoked));
    }

    #[test]
    fn duplicate_or_out_of_order_kernel_receipt_poisoned_epoch() {
        for invalid in [100u32,99,98,0] {
            let mut ledger=NativeFlipLedger::default();
            ledger.begin_flip().unwrap();
            ledger.complete_flip(100).unwrap();
            ledger.begin_flip().unwrap();
            assert_eq!(ledger.complete_flip(invalid),
                       Err(FlipLedgerError::StaleOrDuplicateVblank));
            assert!(ledger.revoked());
            assert_eq!(ledger.begin_flip(),Err(FlipLedgerError::Revoked));
        }
    }

    #[test]
    fn accepts_vblank_wraparound_but_rejects_huge_ambiguous_jump() {
        let mut ledger=NativeFlipLedger::default();
        for counter in [u32::MAX-1,u32::MAX,0,3] {
            ledger.begin_flip().unwrap();
            ledger.complete_flip(counter).unwrap();
        }
        assert_eq!(ledger.completed_count(),4);
        ledger.begin_flip().unwrap();
        assert_eq!(ledger.complete_flip(0x8000_0003),
                   Err(FlipLedgerError::StaleOrDuplicateVblank));
    }

    #[test]
    fn thousands_of_sequential_presentations_without_qemu_budget() {
        let mut ledger=NativeFlipLedger::default();
        for frame in 1..=4096 {
            ledger.begin_flip().unwrap();
            ledger.complete_flip(frame).unwrap();
        }
        assert_eq!(ledger.completed_count(),4096);
        assert!(!ledger.revoked());
    }

    #[test]
    fn virtio_can_report_repeated_or_reset_counters_with_real_events() {
        let mut ledger=NativeFlipLedger::default();
        for frame in [100u32,100,99,99,101] {
            ledger.begin_flip().unwrap();
            ledger.complete_vm_flip(frame).unwrap();
        }
        assert_eq!(ledger.completed_count(),5);
        assert!(ledger.unreliable_vm_counter());
        assert!(!ledger.has_pending());
        assert!(!ledger.revoked());
    }

    #[test]
    fn vm_counter_compatibility_never_invents_an_event_or_rearms_failure() {
        let mut ledger=NativeFlipLedger::default();
        assert_eq!(ledger.complete_vm_flip(1),Err(FlipLedgerError::UnexpectedCompletion));
        assert!(ledger.revoked());
        let mut ledger=NativeFlipLedger::default();
        ledger.begin_flip().unwrap();
        ledger.complete_vm_flip(0).unwrap();
        assert_eq!(ledger.complete_vm_flip(0),Err(FlipLedgerError::UnexpectedCompletion));
        assert!(ledger.revoked());
        ledger.revoke();
        assert_eq!(ledger.begin_flip(),Err(FlipLedgerError::Revoked));
        assert_eq!(ledger.complete_vm_flip(2),Err(FlipLedgerError::Revoked));
    }

    #[test]
    fn revocation_drops_outstanding_flip_and_requires_fresh_ledger() {
        let mut ledger=NativeFlipLedger::default();
        ledger.begin_flip().unwrap();
        ledger.revoke();
        assert_eq!(ledger.complete_flip(2),Err(FlipLedgerError::Revoked));
        assert_eq!(ledger.begin_flip(),Err(FlipLedgerError::Revoked));
        let mut fresh=NativeFlipLedger::default();
        fresh.begin_flip().unwrap();
        fresh.complete_flip(2).unwrap();
        assert_eq!(fresh.completed_count(),1);
    }
}
