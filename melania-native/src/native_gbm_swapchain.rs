//! Per-output, two-slot GBM scanout ownership for real KMS rendering.
//!
//! The GPU owns actual DMA-BUF and DRM FB_ID objects in native_gbm_scanout;
//! this small state machine guards their selection and reuse per CRTC.
//! Never render into the current scanout buffer. The *caller* must wait for
//! the actual kernel PageFlip event and independently verify FB_ID/connector
//! routing BEFORE calling confirm_scanned. The state machine cannot fake that
//! verification and is not a compositor or a native-login authorization.

use drm::control::{crtc,framebuffer};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SwapchainError {
    DuplicateFramebuffer,
    Revoked,
    FramePending,
    NoPendingFrame,
    WrongOutputOrFramebuffer,
    CounterExhausted,
}

#[derive(Debug)]
pub struct NativeGbmSwapchain {
    crtc: crtc::Handle,
    buffers: [framebuffer::Handle;2],
    current: Option<usize>,
    reserved: Option<usize>,
    confirmed: u64,
    revoked: bool,
}

impl NativeGbmSwapchain {
    pub fn new(
        crtc:crtc::Handle,
        first:framebuffer::Handle,
        second:framebuffer::Handle,
    )->Result<Self,SwapchainError> {
        if first==second {return Err(SwapchainError::DuplicateFramebuffer);}
        Ok(Self{
            crtc,buffers:[first,second],current:None,
            reserved:None,confirmed:0,revoked:false,
        })
    }

    pub fn crtc(&self)->crtc::Handle {self.crtc}
    pub fn current(&self)->Option<usize> {self.current}
    pub fn confirmed(&self)->u64 {self.confirmed}
    pub fn revoked(&self)->bool {self.revoked}

    /// Reserve the opposite buffer to the one the CRTC is currently using.
    /// Refuse a second reservation while one is awaiting kernel confirmation.
    pub fn reserve_next(&mut self)->Result<usize,SwapchainError> {
        if self.revoked {return Err(SwapchainError::Revoked);}
        if self.reserved.is_some() {return Err(SwapchainError::FramePending);}
        let slot=match self.current {None|Some(1)=>0,Some(0)=>1,_=>unreachable!()};
        self.reserved=Some(slot);
        Ok(slot)
    }

    pub fn framebuffer(&self,slot:usize)->Option<framebuffer::Handle> {
        self.buffers.get(slot).copied()
    }

    /// Called strictly AFTER a matching DRM event (except initial modeset)
    /// AND a verified live scanout route on the same CRTC. The supplied FB_ID
    /// is the kernel-confirmed buffer, not an IPC promise from a Wayland client.
    pub fn confirm_scanned(
        &mut self,crtc:crtc::Handle,fb:framebuffer::Handle,
    )->Result<usize,SwapchainError> {
        if self.revoked {return Err(SwapchainError::Revoked);}
        let slot=self.reserved.ok_or(SwapchainError::NoPendingFrame)?;
        if crtc!=self.crtc || fb!=self.buffers[slot] {
            self.revoke();
            return Err(SwapchainError::WrongOutputOrFramebuffer);
        }
        let Some(count)=self.confirmed.checked_add(1) else {
            self.revoke();
            return Err(SwapchainError::CounterExhausted);
        };
        self.current=Some(slot);
        self.reserved=None;
        self.confirmed=count;
        Ok(slot)
    }

    /// VT pause, GPU hotplug or lost DRM/kernel frame receipt invalidates
    /// the entire scanout generation. A new libseat/GBM acquisition must
    /// allocate a completely new instance instead of re-arming this one.
    pub fn revoke(&mut self) {
        self.revoked=true;
        self.reserved=None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drm::control;

    fn c(id:u32)->crtc::Handle {control::from_u32(id).unwrap()}
    fn fb(id:u32)->framebuffer::Handle {control::from_u32(id).unwrap()}

    #[test]
    fn twelve_presentations_reuse_two_owned_kernel_framebuffers() {
        let mut output=NativeGbmSwapchain::new(c(11),fb(51),fb(52)).unwrap();
        for index in 0..12 {
            let slot=output.reserve_next().unwrap();
            assert_eq!(slot,index%2);
            assert_eq!(output.reserve_next(),Err(SwapchainError::FramePending));
            assert_eq!(output.confirm_scanned(c(11),fb(51+slot as u32)),Ok(slot));
            assert_eq!(output.current(),Some(slot));
        }
        assert_eq!(output.confirmed(),12);
        assert!(!output.revoked());
    }

    #[test]
    fn never_render_to_displayed_buffer_or_accept_duplicate_framebuffer() {
        assert!(NativeGbmSwapchain::new(c(1),fb(1),fb(1)).is_err());
        let mut output=NativeGbmSwapchain::new(c(1),fb(3),fb(4)).unwrap();
        assert_eq!(output.confirm_scanned(c(1),fb(3)),
                   Err(SwapchainError::NoPendingFrame));
        assert_eq!(output.reserve_next(),Ok(0));
        output.confirm_scanned(c(1),fb(3)).unwrap();
        assert_eq!(output.reserve_next(),Ok(1));
        assert_eq!(output.framebuffer(1),Some(fb(4)));
        assert_eq!(output.framebuffer(2),None);
    }

    #[test]
    fn mismatched_monitor_or_stale_framebuffer_poison_epoch() {
        for (crtc,framebuffer) in [(c(2),fb(51)),(c(1),fb(52))] {
            let mut output=NativeGbmSwapchain::new(c(1),fb(51),fb(52)).unwrap();
            output.reserve_next().unwrap();
            assert_eq!(output.confirm_scanned(crtc,framebuffer),
                       Err(SwapchainError::WrongOutputOrFramebuffer));
            assert!(output.revoked());
            assert_eq!(output.reserve_next(),Err(SwapchainError::Revoked));
        }
    }

    #[test]
    fn two_outputs_hold_independent_slots_and_epochs() {
        let mut a=NativeGbmSwapchain::new(c(10),fb(100),fb(101)).unwrap();
        let mut b=NativeGbmSwapchain::new(c(20),fb(200),fb(201)).unwrap();
        a.reserve_next().unwrap();
        b.reserve_next().unwrap();
        a.confirm_scanned(c(10),fb(100)).unwrap();
        assert_eq!(a.current(),Some(0));
        assert_eq!(b.current(),None);
        b.confirm_scanned(c(20),fb(200)).unwrap();
        a.revoke();
        assert!(a.revoked());
        assert!(!b.revoked());
        assert_eq!(b.reserve_next(),Ok(1));
    }
}
