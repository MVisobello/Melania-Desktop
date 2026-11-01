//! Stage 19: an explicit, read-only audit of *missing* KMS rollback resources.
//!
//! An observed FB_ID is not a framebuffer lease. Copied MODE_ID blob bytes
//! are not owned kernel mode blobs. Therefore this module NEVER authorizes
//! a visible KMS commit, and it does not provide DRM mutation APIs.
use crate::{kms_restore::AtomicBaseline, kms_resources::KmsResourceInventory};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackBlocker {
    OriginalFramebufferOwnershipNotImplemented,
    OriginalKernelModeBlobOwnershipNotImplemented,
    AtomicRollbackNotImplemented,
    VtRecoveryWithLiveScanoutNotVerified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollbackAudit {
    pub active_crtcs: usize,
    pub observed_fb_bindings: usize,
    pub copied_kernel_blob_payloads: usize,
    blockers: [RollbackBlocker; 4],
}

impl RollbackAudit {
    /// Check consistency of the read-only inventory. Never infer that a
    /// nonzero kernel object ID grants ownership or is safe to reuse.
    pub fn inspect(
        baseline: &AtomicBaseline,
        inventory: &KmsResourceInventory,
    ) -> Result<Self, String> {
        baseline.validate()?;
        let mut active_crtcs = 0usize;
        for object in &baseline.objects {
            if object.kind != "crtc" {continue;}
            let active = object.properties.iter().find(|p| p.name=="ACTIVE")
                .ok_or("CRTC ACTIVE property unavailable")?.value;
            let mode = object.properties.iter().find(|p| p.name=="MODE_ID")
                .ok_or("CRTC MODE_ID property unavailable")?.value;
            if active > 1 { return Err("CRTC ACTIVE property is not boolean".into()); }
            if active == 1 {
                active_crtcs += 1;
                if mode == 0 {return Err("Active CRTC has no mode blob".into());}
                if !inventory.blobs.iter().any(|b|
                    b.kind=="crtc" && b.object==object.handle &&
                    b.property=="MODE_ID" && b.original_id==mode && !b.bytes.is_empty()
                ) {
                    return Err("Missing read-only active mode payload".into());
                }
            }
        }

        let mut bindings=0usize;
        for obj in &baseline.objects {
            if obj.kind!="plane" {continue;}
            let fb=obj.properties.iter().find(|p|p.name=="FB_ID")
                .ok_or("Plane framebuffer property unavailable")?.value;
            if fb == 0 {continue;}
            bindings+=1;
            if !inventory.framebuffer_references.iter().any(|r|
                r.plane==obj.handle && r.original_id==fb) {
                return Err("Old framebuffer binding changed during audit".into());
            }
        }
        if bindings!=inventory.framebuffer_references.len() {
            return Err("Unexpected borrowed framebuffer references".into());
        }

        Ok(Self {
            active_crtcs,
            observed_fb_bindings:bindings,
            copied_kernel_blob_payloads:inventory.copied_blob_count(),
            blockers:[
                RollbackBlocker::OriginalFramebufferOwnershipNotImplemented,
                RollbackBlocker::OriginalKernelModeBlobOwnershipNotImplemented,
                RollbackBlocker::AtomicRollbackNotImplemented,
                RollbackBlocker::VtRecoveryWithLiveScanoutNotVerified,
            ],
        })
    }

    pub fn blocker_count(&self) -> usize {self.blockers.len()}
    pub fn blockers(&self) -> &[RollbackBlocker] {&self.blockers}
    pub fn visible_kms_commit_allowed(&self) -> bool {false}
    pub fn rollback_implemented(&self) -> bool {false}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kms_resources::capture_with;
    use crate::kms_restore::{AtomicObject, AtomicProperty};

    fn object(kind:&'static str,handle:&str,props:&[(&str,u64)]) -> AtomicObject {
        AtomicObject {
            kind,handle:handle.into(),
            properties:props.iter().enumerate().map(|(index,(name,value))| AtomicProperty {
                handle:format!("p{index}"),name:(*name).into(),value:*value,
            }).collect()
        }
    }
    fn fixture() -> AtomicBaseline {
        AtomicBaseline {objects:vec![
            object("crtc","crtc0",&[("ACTIVE",1),("MODE_ID",21)]),
            object("connector","connector0",&[("CRTC_ID",5)]),
            object("plane","plane0",&[
                ("CRTC_ID",5),("FB_ID",90),("SRC_X",0),("SRC_Y",0),
                ("SRC_W",640),("SRC_H",480),("CRTC_X",0),("CRTC_Y",0),
                ("CRTC_W",640),("CRTC_H",480)
            ])
        ]}
    }
    #[test]
    fn valid_observations_still_have_four_hard_blockers() {
        let before=fixture();
        let copy=capture_with(&before, |_|Ok(vec![21;64])).unwrap();
        let result=RollbackAudit::inspect(&before,&copy).unwrap();
        assert_eq!(result.active_crtcs,1);
        assert_eq!(result.observed_fb_bindings,1);
        assert_eq!(result.copied_kernel_blob_payloads,1);
        assert_eq!(result.blocker_count(),4);
        assert!(!result.visible_kms_commit_allowed());
        assert!(!result.rollback_implemented());
    }
    #[test]
    fn inactive_output_does_not_make_a_rollback_legal() {
        let mut before=fixture();
        before.objects[0].properties.iter_mut().find(|p|p.name=="ACTIVE")
            .unwrap().value=0;
        before.objects[0].properties.iter_mut().find(|p|p.name=="MODE_ID")
            .unwrap().value=0;
        before.objects[2].properties.iter_mut().find(|p|p.name=="FB_ID")
            .unwrap().value=0;
        let copy=capture_with(&before, |_|Err("unexpected blob".into())).unwrap();
        let audit=RollbackAudit::inspect(&before,&copy).unwrap();
        assert_eq!(audit.active_crtcs,0);
        assert_eq!(audit.observed_fb_bindings,0);
        assert_eq!(audit.blocker_count(),4);
        assert!(!audit.visible_kms_commit_allowed());
    }
    #[test]
    fn missing_owned_mode_payload_is_rejected() {
        let before=fixture();
        let mut copy=capture_with(&before, |_|Ok(vec![21])).unwrap();
        copy.blobs.clear();
        assert!(RollbackAudit::inspect(&before,&copy).is_err());
    }
    #[test]
    fn framebuffer_binding_mismatch_is_rejected() {
        let before=fixture();
        let mut copy=capture_with(&before, |_|Ok(vec![21])).unwrap();
        copy.framebuffer_references[0].original_id=999;
        assert!(RollbackAudit::inspect(&before,&copy).is_err());
    }
    #[test]
    fn malformed_active_crtc_is_rejected() {
        let mut before=fixture();
        before.objects[0].properties.iter_mut().find(|p|p.name=="ACTIVE")
            .unwrap().value=2;
        let copy=capture_with(&before, |_|Ok(vec![21])).unwrap();
        assert!(RollbackAudit::inspect(&before,&copy).is_err());
    }
    #[test]
    fn even_a_full_copied_snapshot_does_not_enable_scanout() {
        let before=fixture();
        let mut copy=capture_with(&before, |_|Ok(vec![21])).unwrap();
        copy.blobs.push(copy.blobs[0].clone());
        let audit=RollbackAudit::inspect(&before,&copy).unwrap();
        assert!(!audit.visible_kms_commit_allowed());
        assert!(audit.blockers().contains(
            &RollbackBlocker::OriginalFramebufferOwnershipNotImplemented));
        assert!(audit.blockers().contains(
            &RollbackBlocker::VtRecoveryWithLiveScanoutNotVerified));
    }
}
