# Stage 44 — versioned continuous KMS transport (not yet connected)

The existing real QEMU renderer uses a deliberately bounded six-frame protocol.
ContinuousRedraw can track more than six scene generations, but the old
AsyncKmsWorker limits its frame ordinal to 1–6.

This stage adds a **separate transport building block** in
melania-native/src/continuous_kms_stream.rs. It is NOT selected by the
native Wayland bridge, does not open DRM devices and does not enable a
native login session.

## New framed protocol

All integers are little-endian; current QEMU test images use 1280 x 800 x 4
BGRA bytes.

| Message | Fixed header | Payload |
| --- | --- | --- |
| Frame | MLF1, epoch u64, ticket sequence u64, scene generation u64, image size u32 | BGRA pixels |
| ACK | MLA1, epoch u64, ticket sequence u64, scene generation u64, status u8 | none |

A successful ACK must match ALL three identifiers and have zero status.
An incomplete, malformed or mismatched ACK poisons this stream and cannot
count as a presented frame. Only one job may be queued. The calling
ContinuousRedraw planner must admit at most one active KMS flip.

Socket I/O runs on a dedicated thread; the eventual Smithay caller only
uses nonblocking submit/try_result.

## Validation in this PR

Rust socket-pair unit tests simulate the KMS peer and verify ten sequential
frames, nonblocking event-loop polling, stale epoch/sequence/generation
rejection, explicit failure replies and invalid image rejection.
These tests do NOT prove actual kernel framebuffer presentation. A real
QEMU receiver must emit a successful MLA1 ACK only after genuine DRM/KMS
CRTC FB_ID/page-flip confirmation.

## Next required stage

1. Implement the matching versioned receiver in disposable QEMU Rust
   DRM/KMS, keeping the existing six-frame path intact.
2. Wire a separate explicitly gated QEMU-only mode to ContinuousRedraw:
   multiple client commits, move, close and reopen cause redraw damage.
3. Require 10+ real ACKs with QMP pixel captures and test coalescing,
   pause/VT, stale ACK and unconditional CRTC restoration.
4. Only after successful VM and hardware safety tests should the old ordinal
   path be replaced and native-session work considered.

Shell, icons, themes, titlebar buttons, window geometry and existing QEMU
proof are untouched. NATIVE_SESSION_READY=false remains authoritative.
