//! Stage 12: bounded, read-only copies of KMS mode/color blob payloads.
//!
//! These bytes may help construct a future rollback request, but they do NOT
//! preserve kernel blob handles, framebuffers, DRM master, or the previous VT.
//! No setter, DRM blob creation, modeset, scanout or restore is implemented.
use std::collections::BTreeSet;
use drm::control::Device as ControlDevice;
use smithay::backend::drm::DrmDeviceFd;
use crate::kms_restore::AtomicBaseline;

const MAX_BLOBS: usize = 128;
const MAX_BLOB_BYTES: usize = 256 * 1024;
const MAX_ALL_BLOB_BYTES: usize = 2 * 1024 * 1024;
const MAX_FRAMEBUFFER_REFS: usize = 128;

#[derive(Clone, PartialEq, Eq)]
pub struct BlobCopy {
    pub kind: &'static str,
    pub object: String,
    pub property: String,
    pub original_id: u64,
    // Owned userspace copy; cannot be used as a live DRM property blob ID.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FramebufferReference {
    pub plane: String,
    pub original_id: u64,
}

/// An observational inventory. The framebuffer IDs are explicitly borrowed
/// observations, NEVER resource ownership or preservation of GEM handles.
#[derive(Clone, PartialEq, Eq)]
pub struct KmsResourceInventory {
    pub blobs: Vec<BlobCopy>,
    pub framebuffer_references: Vec<FramebufferReference>,
}

fn copied_blob(kind: &str, name: &str) -> bool {
    match kind {
        "crtc" => matches!(name, "MODE_ID" | "GAMMA_LUT" | "DEGAMMA_LUT" | "CTM"),
        "connector" => matches!(name, "HDR_OUTPUT_METADATA"),
        _ => false,
    }
}

impl KmsResourceInventory {
    /// Even after all payloads are copied, old framebuffer storage and
    /// kernel blob object lifetimes are not retained by this inventory.
    pub fn restoration_implemented(&self) -> bool { false }

    pub fn unchanged_since(&self, earlier: &Self) -> bool {
        self == earlier
    }

    pub fn copied_blob_count(&self) -> usize { self.blobs.len() }
    pub fn borrowed_framebuffer_count(&self) -> usize {
        self.framebuffer_references.len()
    }
}

/// Uses a caller-supplied, *read-only* blob reader so the selection and memory
/// limits can be tested without opening a DRM card or creating test devices.
///
/// No blob data is written to a log or persisted on disk; it only lives as
/// long as the caller keeps this structure in memory.
pub fn capture_with<F>(
    baseline: &AtomicBaseline,
    mut read_blob: F,
) -> Result<KmsResourceInventory, String>
where
    F: FnMut(u64) -> Result<Vec<u8>, String>,
{
    baseline.validate()?;
    let mut blobs = Vec::new();
    let mut framebuffer_references = Vec::new();
    let mut observed_fb = BTreeSet::new();
    let mut total_bytes = 0usize;

    for object in &baseline.objects {
        if object.kind == "crtc" {
            let active = object.properties.iter()
                .find(|p| p.name == "ACTIVE").map(|p| p.value).unwrap_or(0);
            let mode_id = object.properties.iter()
                .find(|p| p.name == "MODE_ID").map(|p| p.value).unwrap_or(0);
            if active != 0 && mode_id == 0 {
                return Err(format!("Active CRTC {} has no MODE_ID", object.handle));
            }
        }
        for property in &object.properties {
            if object.kind == "plane" && property.name == "FB_ID" && property.value != 0 {
                if !observed_fb.insert((object.handle.clone(), property.value)) {
                    return Err("Duplicate framebuffer binding".into());
                }
                if framebuffer_references.len() >= MAX_FRAMEBUFFER_REFS {
                    return Err("Too many borrowed framebuffer references".into());
                }
                framebuffer_references.push(FramebufferReference {
                    plane: object.handle.clone(), original_id: property.value,
                });
            }
            if !copied_blob(object.kind, &property.name) || property.value == 0 {
                continue;
            }
            if blobs.len() >= MAX_BLOBS {
                return Err("Too many KMS property blobs".into());
            }
            let bytes = read_blob(property.value)
                .map_err(|e|format!("Cannot read {} blob: {e}",property.name))?;
            if bytes.is_empty() || bytes.len() > MAX_BLOB_BYTES {
                return Err(format!("Invalid {} blob length: {}",property.name,bytes.len()));
            }
            total_bytes=total_bytes.checked_add(bytes.len())
                .ok_or_else(||"KMS blob size overflow".to_string())?;
            if total_bytes > MAX_ALL_BLOB_BYTES {
                return Err("KMS blob inventory exceeds memory budget".into());
            }
            blobs.push(BlobCopy {
                kind: object.kind, object: object.handle.clone(),
                property: property.name.clone(), original_id: property.value, bytes,
            });
        }
    }
    blobs.sort_by(|a,b| (a.kind,&a.object,&a.property)
        .cmp(&(b.kind,&b.object,&b.property)));
    framebuffer_references.sort_by(|a,b| (&a.plane,a.original_id)
        .cmp(&(&b.plane,b.original_id)));
    Ok(KmsResourceInventory {blobs,framebuffer_references})
}

/// Only usable inside the existing, explicitly QEMU- and TTY-gated preflight.
/// get_property_blob reads the existing immutable kernel payload: it does NOT
/// keep the original blob ID alive, nor create/modify a DRM resource.
pub fn capture_read_only(
    fd: &DrmDeviceFd,
    baseline: &AtomicBaseline,
) -> Result<KmsResourceInventory, String> {
    capture_with(baseline, |id|fd.get_property_blob(id)
        .map_err(|e|format!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kms_restore::{AtomicObject,AtomicProperty};
    fn object(kind:&'static str,handle:&str,props:&[(&str,u64)])->AtomicObject {
        AtomicObject {
            kind,handle:handle.into(),
            properties:props.iter().enumerate().map(|(n,(name,value))|
                AtomicProperty {
                    handle:format!("p{n}"),name:(*name).into(),value:*value,
                }).collect(),
        }
    }
    fn fixture()->AtomicBaseline {
        AtomicBaseline {objects:vec![
            object("crtc","crtc0",&[("ACTIVE",1),("MODE_ID",21),("GAMMA_LUT",22)]),
            object("connector","connector0",&[("CRTC_ID",1),("HDR_OUTPUT_METADATA",23)]),
            object("plane","plane0",&[
                ("CRTC_ID",1),("FB_ID",90),("SRC_X",0),("SRC_Y",0),
                ("SRC_W",10),("SRC_H",10),("CRTC_X",0),("CRTC_Y",0),
                ("CRTC_W",640),("CRTC_H",480),
            ]),
        ]}
    }
    #[test]
    fn captures_mode_color_hdr_bytes_and_borrowed_fb_ids_only() {
        let before=capture_with(&fixture(),|id|Ok(vec![id as u8;64])).unwrap();
        assert_eq!(before.copied_blob_count(),3);
        assert_eq!(before.borrowed_framebuffer_count(),1);
        assert_eq!(before.framebuffer_references[0].original_id,90);
        assert!(!before.restoration_implemented());
        let after=capture_with(&fixture(),|id|Ok(vec![id as u8;64])).unwrap();
        assert!(after.unchanged_since(&before));
        assert!(before.blobs.iter().any(|b|b.property=="MODE_ID" && b.bytes[0]==21));
    }
    #[test]
    fn detects_changes_in_payload_even_if_blob_ids_are_unchanged() {
        let old=capture_with(&fixture(),|id|Ok(vec![id as u8;64])).unwrap();
        let new=capture_with(&fixture(),|id|Ok(vec![id as u8 ^ 1;64])).unwrap();
        assert!(!new.unchanged_since(&old));
        assert!(!old.restoration_implemented());
    }
    #[test]
    fn rejects_missing_active_mode_and_unreadable_blob() {
        let mut broken=fixture();
        broken.objects[0].properties.iter_mut()
            .find(|p|p.name=="MODE_ID").unwrap().value=0;
        assert!(capture_with(&broken, |_|Ok(vec![1])).is_err());
        assert!(capture_with(&fixture(),|_|Err("missing".into())).is_err());
    }
    #[test]
    fn bounded_blob_copies_reject_empty_and_excessive_payloads() {
        assert!(capture_with(&fixture(),|_|Ok(Vec::new())).is_err());
        assert!(capture_with(&fixture(),|_|Ok(vec![0;MAX_BLOB_BYTES+1])).is_err());
        assert!(capture_with(&fixture(),|_|Ok(vec![1;MAX_BLOB_BYTES])).is_ok());
    }
    #[test]
    fn zero_fb_is_not_a_retained_framebuffer_and_no_blob_is_created() {
        let mut zero=fixture();
        zero.objects[2].properties.iter_mut()
            .find(|p|p.name=="FB_ID").unwrap().value=0;
        let copy=capture_with(&zero,|_|Ok(vec![1,2,3])).unwrap();
        assert_eq!(copy.borrowed_framebuffer_count(),0);
    }
}
