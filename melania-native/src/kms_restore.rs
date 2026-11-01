//! Stage 11: read-only atomic KMS baseline for an eventual rollback design.
//!
//! No restore API is provided: a snapshot of DRM property IDs/values is not
//! sufficient to restore framebuffer lifetimes, property blobs or a VT.
//! This module only reads live state and verifies that a QEMU TEST_ONLY
//! experiment left that observed state unchanged.
use std::{collections::BTreeSet, fmt::Debug};
use drm::control::{Device as ControlDevice, ResourceHandle};
use smithay::backend::drm::DrmDeviceFd;

const MAX_CRTCS: usize = 16;
const MAX_CONNECTORS: usize = 32;
const MAX_PLANES: usize = 128;
const MAX_PROPERTIES_PER_OBJECT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicProperty {
    pub handle: String,
    pub name: String,
    pub value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicObject {
    pub kind: &'static str,
    pub handle: String,
    pub properties: Vec<AtomicProperty>,
}

impl AtomicObject {
    /// Missing or repeated critical properties invalidate a baseline rather
    /// than silently recording an incomplete set of planes/CRTC bindings.
    fn validate(&self) -> Result<(), String> {
        if self.properties.is_empty() || self.properties.len() > MAX_PROPERTIES_PER_OBJECT {
            return Err(format!("Invalid {} property count: {}", self.kind, self.properties.len()));
        }
        let mut names = BTreeSet::new();
        let mut handles = BTreeSet::new();
        for property in &self.properties {
            if property.name.is_empty() || property.name.len() > 128 ||
                !names.insert(property.name.as_str()) ||
                !handles.insert(property.handle.as_str()) {
                return Err(format!("Duplicate or malformed {} property", self.kind));
            }
        }
        let required: &[&str] = match self.kind {
            "crtc" => &["ACTIVE", "MODE_ID"],
            "connector" => &["CRTC_ID"],
            "plane" => &["CRTC_ID", "FB_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H",
                "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"],
            _ => return Err("Unknown KMS object kind".into()),
        };
        for name in required {
            if !names.contains(name) {
                return Err(format!("Missing {} atomic property {name}", self.kind));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicBaseline {
    pub objects: Vec<AtomicObject>,
}

impl AtomicBaseline {
    fn validate(&self) -> Result<(), String> {
        if self.objects.is_empty() ||
            !self.objects.iter().any(|o| o.kind == "crtc") ||
            !self.objects.iter().any(|o| o.kind == "connector") ||
            !self.objects.iter().any(|o| o.kind == "plane") {
            return Err("Incomplete KMS baseline: CRTC, connector and plane required".into());
        }
        let mut ids = BTreeSet::new();
        for object in &self.objects {
            object.validate()?;
            if !ids.insert((object.kind, object.handle.as_str())) {
                return Err("Duplicate KMS object".into());
            }
        }
        Ok(())
    }

    /// Changes in object count, IDs, bindings or any property value all fail
    /// closed, including hotplug and changes caused by another KMS client.
    pub fn unchanged_since(&self, before: &Self) -> bool {
        self.validate().is_ok() && before.validate().is_ok() && self == before
    }

    /// Deliberate hard gate: raw property snapshots do NOT own the old FBs,
    /// mode blobs, VT or a working atomic rollback operation.
    pub fn restoration_implemented(&self) -> bool { false }
}

fn capture_object<T: ResourceHandle + Debug>(
    fd: &DrmDeviceFd, kind: &'static str, handle: T,
) -> Result<AtomicObject, String> {
    let label = format!("{handle:?}");
    let properties = fd.get_properties(handle)
        .map_err(|e| format!("Cannot read {kind} {label} properties: {e}"))?;
    if properties.iter().count() > MAX_PROPERTIES_PER_OBJECT {
        return Err(format!("Excessive {kind} properties at {label}"));
    }
    let mut list = Vec::new();
    for (&id, &value) in properties.iter() {
        let info = fd.get_property(id)
            .map_err(|e| format!("Cannot describe {kind} property {id:?}: {e}"))?;
        let name = info.name().to_str()
            .map_err(|e| format!("Invalid {kind} property name: {e}"))?
            .to_owned();
        list.push(AtomicProperty { handle: format!("{id:?}"), name, value });
    }
    list.sort_by(|a,b| a.name.cmp(&b.name));
    let item = AtomicObject {kind, handle:label, properties:list};
    item.validate()?;
    Ok(item)
}

/// Read all CRTC, connector and plane properties, not just the chosen output.
/// Always called after the existing dedicated QEMU/TTY/libseat safety gates.
/// No DRM set_property/atomic_commit, framebuffer or blob allocation occurs.
pub fn capture_read_only(fd: &DrmDeviceFd) -> Result<AtomicBaseline, String> {
    let resources = fd.resource_handles()
        .map_err(|e| format!("Cannot enumerate DRM resources: {e}"))?;
    let planes = fd.plane_handles()
        .map_err(|e| format!("Cannot enumerate DRM planes: {e}"))?;
    if resources.crtcs().len() > MAX_CRTCS ||
        resources.connectors().len() > MAX_CONNECTORS ||
        planes.len() > MAX_PLANES {
        return Err("KMS baseline exceeds bounded object count".into());
    }
    let mut objects = Vec::new();
    for &handle in resources.crtcs() {
        objects.push(capture_object(fd, "crtc", handle)?);
    }
    for &handle in resources.connectors() {
        objects.push(capture_object(fd, "connector", handle)?);
    }
    for handle in planes {
        objects.push(capture_object(fd, "plane", handle)?);
    }
    objects.sort_by(|a,b| (a.kind, &a.handle).cmp(&(b.kind, &b.handle)));
    let baseline = AtomicBaseline {objects};
    baseline.validate()?;
    Ok(baseline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(kind:&'static str, id:&str, names:&[&str]) -> AtomicObject {
        AtomicObject {
            kind, handle:id.to_owned(),
            properties:names.iter().enumerate().map(|(n, name)| AtomicProperty {
                handle:format!("p{n}"), name:(*name).to_owned(), value:n as u64,
            }).collect(),
        }
    }
    fn baseline() -> AtomicBaseline {
        AtomicBaseline {objects: vec![
            object("crtc", "crtc0", &["ACTIVE", "MODE_ID"]),
            object("connector", "connector0", &["CRTC_ID"]),
            object("plane", "plane0", &[
                "CRTC_ID", "FB_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H",
                "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
            ]),
        ]}
    }
    #[test]
    fn valid_baseline_can_be_compared_but_never_restored() {
        let before=baseline();
        assert!(before.validate().is_ok());
        assert!(before.unchanged_since(&before.clone()));
        assert!(!before.restoration_implemented());
        assert!(!crate::renderer::native_scanout_is_implemented());
    }
    #[test]
    fn property_mutation_and_hotplug_are_detected() {
        let before=baseline();
        let mut changed=before.clone();
        changed.objects[0].properties[0].value+=1;
        assert!(!changed.unchanged_since(&before));
        let mut unplugged=before.clone();
        unplugged.objects.remove(1);
        assert!(!unplugged.unchanged_since(&before));
    }
    #[test]
    fn rejects_duplicate_properties_and_missing_atomic_bindings() {
        let mut missing=baseline();
        missing.objects[2].properties.retain(|p| p.name != "FB_ID");
        assert!(missing.validate().is_err());
        let mut duplicate=baseline();
        duplicate.objects[0].properties.push(duplicate.objects[0].properties[0].clone());
        assert!(duplicate.validate().is_err());
        let mut repeated=baseline();
        repeated.objects.push(repeated.objects[1].clone());
        assert!(repeated.validate().is_err());
    }
    #[test]
    fn rejects_empty_or_partial_snapshots() {
        assert!(AtomicBaseline {objects:vec![]}.validate().is_err());
        let mut partial=baseline();
        partial.objects.retain(|o|o.kind!="plane");
        assert!(partial.validate().is_err());
        let mut huge=baseline();
        let item=&mut huge.objects[0];
        for n in 2..=MAX_PROPERTIES_PER_OBJECT {
            item.properties.push(AtomicProperty {
                handle:format!("p{n}"),name:format!("EXTRA_{n}"),value:n as u64,
            });
        }
        assert!(huge.validate().is_err());
    }
}
