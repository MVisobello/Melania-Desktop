# Stage 93 — verify original connector-to-CRTC routing after QEMU KMS rollback

The Stage91 DRM receiver already checked original mode, CRTC position and
framebuffer identity after the final frame. These checks alone did **not**
prove that the virtual monitor was attached to the original CRTC again.

## Implementation

The **disposable QEMU** `qemu-rust-kms-continuous` restore guard now:

- reads the connector's current encoder after restoring the original CRTC;
- verifies that encoder routes back to the original CRTC handle;
- includes that route in the existing `MELANIA_RUST_KMS_RESTORE_VERIFIED` condition;
- arms its rollback guard **before** the first mutating `set_crtc`, so a
  failed modeset does not silently skip the best-effort rollback attempt.

The QEMU guest harness demands
`MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true` in all eight scenarios
using the continuous DRM receiver. Any missing or false evidence fails the VM
workflow rather than declaring the screen recovered.

## Verification

- `python3 -m unittest discover -s tests -p test_native_kms_restore_stage93.py -v`
- `cargo test -p melania-native --all-targets`
- **Real QEMU CI** on the exact PR head is required before integration.

## Boundaries

The program uses the existing active framebuffer on the **virtual QEMU
GPU**. This is not a kernel-backed atomic rollback engine, GPU hotplug
recovery, multiprocess DRM master ownership, multi-monitor support or VT
restoration following a crash. The original framebuffer lifetime still
depends on the guest console remaining alive.

Do **not** extrapolate this check to host DRM or a login-manager session.
`NATIVE_SESSION_READY=false`.
