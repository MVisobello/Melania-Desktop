# Stage 98 — nonblocking native panel IPC on the Smithay event loop

## Problem

The experimental Stage 97 native Wayland control socket previously called a blocking `read_command` on each accepted connection. The 40 ms per-client timeout multiplied by eight accepted clients could delay Wayland input, libinput and DRM/KMS completion handling even if those clients never wrote a complete request. That is incompatible with an interactive compositor.

## Change

- `NativeControlPoller` accepts at most eight connections per tick and retains at most 16 pending peers; excess connections are closed.
- Every client is `O_NONBLOCK`, with at most two read and two write operations per pass and a two-second maximum lifetime.
- Commands remain limited to the existing `melania-control` `MAX_COMMAND` (64 bytes), require one complete LF line and write-half EOF, and reject multiple commands, invalid UTF-8 and partial data.
- Invalid and oversized input generates an explicit bounded error; replies are capped by `MAX_RESPONSE` (128 KiB) and written incrementally, without blocking on slow readers.
- The poller invokes the **existing** `BridgeState::control_command` on the compositor thread only after receiving an entire valid request. There is no shell execution, unbounded worker spawning or new IPC protocol.
- The socket continues to use the private `XDG_RUNTIME_DIR` and filesystem permissions 0600. It is created only by the explicitly gated QEMU managed-xdg scenario, not a production session.

## Tests

Rust tests exercise simultaneous stalled/complete clients, isolation from slow peers, bounded concurrent connections, oversized requests, missing newline, invalid UTF-8 and multiple-command injection. Existing QEMU Stage97 must additionally confirm that the actual `LIST → ACTIVATE → LIST` Wayland roundtrip still works with the nonblocking server.

## Remaining prerequisites

This fix improves input responsiveness and protocol safety; it **does not** establish live KMS pixel proof of minimized/restored windows, eliminate the QEMU presentation-frame cap or make the Rust compositor safe for a display-manager login. `NATIVE_SESSION_READY=false`.
