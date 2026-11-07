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
