# Stage132 — XDG group resize transaction ledger

Adds a pure Rust *fail-closed* compositor policy for resizing real tab stacks.
A group occupies one tile, but each constituent live XDG toplevel still
requires its own configure serial, matching ack_configure, and a **new
validated owned wl_shm buffer of the requested dimensions**. Only when
all groups/members pass can the compositor atomically publish the tiled plan.

It tracks one bounded output-wide transaction (up to 32 clients) with
monotonically increasing generation IDs, rejects bad serials, unregistered
or duplicate clients, wrong buffer dimensions and old generations; it aborts
on a client disappearing or libseat/VT/DRM ownership revocation. A second
configure revokes prior ACK and buffer acceptance for that client.

**Acceptance boundary:** the Rust ledger is added and exercised by unit
tests. It is *not yet connected to live Smithay XDG configure-ACK events*.
The native bridge continues to prohibit simultaneous
`MELANIA_NATIVE_TAB_STACKS` and `MELANIA_NATIVE_AUTO_TILING`. The next
step is to wire real XDG serials, ACK and client buffer commits, conceal
the incomplete transaction, and add an independent two-client QEMU resize
with physical CRTC pixels. These tests are not a substitute for physical
multi-monitor/VT acceptance.

Keep `NATIVE_SESSION_READY=false`; preserve labwc fallback.

## Smithay 0.7 event-wiring follow-up

The QEMU Wayland bridge now owns a default-inactive transaction ledger.
Smithay's real `XdgShellHandler::ack_configure` callback forwards the
acknowledged `ToplevelConfigure.serial` (never a synthetic receipt).
Only after an accepted bounded wl_shm image enters the scene can its size
be forwarded as a commit receipt. Destroying any participating toplevel,
VT/libseat pause or GPU/udev ownership revocation aborts the pending ledger.
An independent structural QEMU CI test enforces these boundaries.

**Still blocked:** creation/arming of a multi-client resize batch,
concealment of all unready images, and atomic commit/presentation of all
resized group members. No production or combined-mode activation is enabled.

## Stage133 retained-scene atomic batch

`DynamicWindowScene::upsert_batch` now validates every owned image and
client ID, scene membership count and the **aggregate** memory budget
before any mutation. After validation it updates all positions, visibility
and client pixels in one scene transaction with **one** generation bump and
both old and new damage rectangles invalidated.

Unit tests prove that inactive tabs never become temporarily visible,
rejected duplicates/IDs cannot partially mutate the scene, and existing
z-order is stable. **No KMS receipt is fabricated.**

Next gate: connect the already-implemented XDG ACK/buffer transaction to
this batch API, stage every owned client image while the group is pending,
then publish only after the whole transaction is ready. Until that is
completed, the live QEMU bridge still refuses simultaneous tab stacks and
auto tiling.

## Stage134 — staged real images, single atomic scene publish

During an active group resize, each genuinely acknowledged, valid owned
wl_shm image is staged outside the visible `DynamicWindowScene`. The
compositor does not perform a per-tab upsert or submit a DRM/KMS frame for
these intermediate commits. Once all group members have committed matching
images, the bridge validates every member and applies one atomic scene batch
with exactly the group's active tabs visible. Retained geometry and pending
resize state are updated only after the batch is accepted.

A destroyed client or VT/GPU revocation clears all staged images without
touching the previous confirmed scene. QEMU group+auto-tiling remains gated
off until live per-member XDG configure dispatch is implemented and tested.

## Stage136 — independent QEMU end-to-end acceptance harness

Added a *separate* guest instance with opt-in combined tab stacks and
tiling. The compositor starts floating despite the configured tiling flag,
then QMP injects genuine **Super+S, Super+Shift+S, Super+Y** key events to
form the group and request tiled XDG geometry. Two real Wayland clients
independently ACK their **actual** configure serials and attach owned
1280×800 wl_shm buffers *without waiting on an intermediate per-client KMS
ACK*. The compositor must produce a single grouped scene publication.

QMP captures and validates the actual VirtIO GPU CRTC image: full-screen
client A after atomic publish, full-screen B after real Super+Left keyboard
focus, and A after Super+Right. The test fails on wrong colors, missing
real wl_keyboard Enter, lack of real frame callbacks, transaction timeout,
GPU framebuffer/frame count mismatch, missing original CRTC/connector
restore or missing libseat release. The host CI checks returned status
and all four screenshot files, not just console markers.

This stage is not accepted until the exact final-head Linux/SFTP/QEMU
workflow is **completed/success** and screenshots are inspected. Actual
hardware, DRM/GBM continuous production, DMA-BUF, multi-output and login
remain blocked. The normal Melania session still uses labwc as fallback.
