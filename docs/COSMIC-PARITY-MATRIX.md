# COSMIC Comp ↔ Melania Native: parity matrix (2026-11-07)

**Goal:** Melania Native must reach at least the **validated end-user feature parity**
of [COSMIC Comp](https://github.com/pop-os/cosmic-comp), while retaining the
Melania UI (Win7-style Start/taskbar and Light/Dark/Aero). This is a living
engineering acceptance matrix, not a release claim.

**Trackers:** [#298 parity epic](https://github.com/MVisobello/Melania-Desktop/issues/298),
[#24 native P0](https://github.com/MVisobello/Melania-Desktop/issues/24),
[#296 true GPU Aero blur](https://github.com/MVisobello/Melania-Desktop/issues/296).

## What is being compared

- **COSMIC Comp** owns Wayland/X11 windows, geometry, outputs, rendering,
  input, focus, workspaces and presentation. COSMIC Desktop includes *other*
  processes for panels, settings, launcher, portals, notifications, files, etc.
- Melania currently has two distinct execution paths:
  melania-compositor is a **working nested winit/Smithay compositor**;
  melania-native has an **isolated native libseat/DRM/KMS/QEMU experimental
  path** with extensive models and hardware-safety tests, but **not yet a
  certified physical standalone login session**.
- A Rust module or a QEMU pixel receipt is **not** equivalent to a feature
  working on dedicated physical hardware. This audit never grants readiness
  without the acceptance evidence of #24.
- Status labels: **N** nested functional; **M** native model or unit test;
  **Q** integrated native QEMU proof (limited); **U** unverified/incomplete in
  native production; **—** not implemented. A cell may contain several labels.

## Compositor parity baseline

| COSMIC Comp capability | Melania evidence / status | Required proof for parity | Priority |
| --- | --- | --- | --- |
| Wayland compositor, wl_surface/xdg_toplevel lifecycle | N; Q partial via melania-native/src/wayland_bridge.rs; U | Continuous multi-client native display, destroy/restart, input/render tests | P0 |
| xdg_popup positioning, authenticated popup grabs, dismissal | N; Q partial; #283 merged | Nested hierarchy, focus and serial tests under normal DRM loop | P0 |
| layer-shell panels, work areas, exclusive zones | N; Q partial | Multi-output layer priority and real panel constraints | P0 |
| Floating window move/resize, maximize, minimize, focus | N; Q partial | Real XDG configuration and client ACK, unfocused protections | P0 |
| Fullscreen and preservation across workspace/output moves | N partial; U | Multi-fullscreen, video/games and focus recovery | P1 |
| Manual snapping to edges/corners | N; M native snap_layout.rs; Q partial | Drag/release on real native XDG scene and every output | P1 |
| **Automatic tiling** independently per workspace | **M in auto_tiling.rs (this change); U** | Register real XDG windows; apply XDG configure; opt-in shortcut, resize, recovery | P1 |
| Tiling orientation, gap spacing, exempt floating apps | **M in auto_tiling.rs; U** | Actual workspace settings + client minimum-size negotiated behavior | P1 |
| Tiling tab/stack groups and reordering/swapping tiles | **M basic swap only; — tab groups** | Real tabbed clients, focus/stack and drag-drop tile swap | P1 |
| Tiling resize/keyboard navigation/tree selection | — | Interactive resize/configure and shortcuts equivalent to COSMIC | P1 |
| Workspace navigation, sending windows to workspace | N via melania-workspaces IPC; Q partial | Native workspace-scoped focus, drag and per-monitor association | P1 |
| Dynamic and pinned workspaces | — | Creation/removal, persistence, pinned vs dynamic, shortcuts | P1 |
| Workspace overview and switching gestures | N shell integration partial; native U | Full native view, keyboard/pointer/touch interaction | P1 |
| Configurable keyboard shortcuts, Alt+Tab, shortcuts for tiles | N Alt+Tab and Ctrl+Alt arrows; Q partial | Native input policy, shortcuts editor/config persistence | P1 |
| Focus follows cursor, cursor follows focus, window raise rules | — configurable policies | Native/libinput integration, focus isolation and popup correctness | P1 |
| XKB layouts, repeat, IME/text-input and input methods | N basic keyboard; native U | Multi-layout, XKB non-Latin, fcitx/IME/client protocol test | P0 |
| Pointer axis, wheel, trackpad, constraints, relative pointer | N wheel work #287 open at audit; native Q partial | Native pointer axis, constraints game tests, tablet gestures | P0 |
| Touch/tablet/gesture and custom touchpad settings | M device enumeration; U | Physical touch/tablet/gesture, libinput policy, config | P1 |
| Clipboard/primary selection and DnD | N wl_data_device; #284 merged native clipboard focus; U | Two-client clipboard, primary selection, DnD transfers and VT revocation | P0 |
| Multiple physical monitors, hotplug/unplug | M topology/CRTC routing and epoch guards (#293 merged); U | Dual hardware monitor live presentation, unplug/replug recovery | P0 |
| Monitor resolution, refresh, rotation, scaling | M modes and scale validation; U | Real per-output mode, transform, scale, fractional-scale protocol | P0 |
| DRM/KMS scanout, GBM/EGL/GLES, CRTC vblank/page flip | Q finite + persistent experimental; #285-#293 merged; U | Long-running real hardware loop and verified release of callbacks | P0 |
| Client DMA-BUF import, modifiers, sync/fences, GPU reset | M renderer/GBM proof; U | Negotiated client buffers + recoverable render/present | P0 |
| Frame scheduling, damage propagation, efficient idle | M/Q partial | Real output-specific damage, idle CPU, resize, storms, benchmark | P0 |
| VRR and gaming-oriented presentation where supported | U | Physical VRR/display path, latency and frame-pacing evidence | P1 |
| XWayland applications and WM bridge | — | Multiple X11 apps, clipboard, DnD, focus, XWayland crash/restart | P0 |
| XDG screenshot, screencopy, screencast/PipeWire portals | N screenshot test infrastructure only; U | Portal permission prompt and capture/isolation, OBS/screenshare | P0 |
| Secure session lock on every physical output | M lock state model; U | Protected content through VT switch/hotplug, authenticated unlock | P0 |
| Login/session manager, DBus, polkit, environment, logout | N preview session; native U | Dedicated session from DM, app lifecycle, safe logout/restart | P0 |
| Hardware idle/power, suspend/resume, VT recovery | M lifecycle; Q partial | Real physical suspend/resume, VT roundtrips and fail-close | P0 |
| Correct graceful shutdown and fallback to labwc | Default labwc; native U | Reproducible failure injection, guest+physical restore, no host takeover | P0 |
| Display zoom/color filters and accessibility compositor effects | — | Actual GPU compositing and keyboard bindings | P1 |
| Corner preferences, shadows, cosmetic window effects | N Aero alpha with #295 merged; native U | Physical rendering/scale/damage, high-contrast accessibility | P1 |

## COSMIC Desktop features (outside cosmic-comp)

These are separate parity workstreams, not compositor features: application
launcher and library/search, panels/dock/applets, notifications/OSD,
file manager, settings, screenshot UI, session greeter/lock UI,
audio/network/Bluetooth/power controls, accessibility tooling, package store
and media/text/terminal applications. Melania already has UI work in several
of these components, but **do not count it as native compositor parity**.
Use separate integration and user acceptance tests.

## COSMIC release vs roadmap

As of this audit the reference list above reflects confirmed public
capabilities, e.g. optional automatic tiling, floating, workspaces,
multi-display/XWayland, pointer constraints and July 2026 window features.
The [System76 Epoch 2/3 roadmap](https://system76.com/blog/post/cosmic-epoch-2-and-3-roadmap)
also names **HDR, Night Light, configurable gestures, session restore and
desktop animations** as planned work. Track these after verifying a shipped
implementation; do not falsely mark planned work as already released COSMIC
functionality.

## First concrete implementation: bounded auto-tiling planner

The new melania-native/src/auto_tiling.rs introduces a **pure Rust, hardware-safe**
automatic tiling model for **one workspace per instance**:

- explicit enable/disable; horizontal/vertical starting orientation;
  configurable 0–32 logical-pixel gap;
- deterministic balanced subdivision with pixel-exact cuts on odd-sized
  areas, panel-reserved work area and bounded minimum dimensions;
- stable window order, swap, floating exclusion, minimize exclusion,
  teardown; all-or-nothing rejection when a layout cannot fit;
- unit tests for 3-window odd dimensions, 5-window separation, exclusion,
  swap, limits, invalid output bounds and atomic failure.

**Not done in this PR:** bind the planner to the live wayland_bridge and
send real xdg_toplevel configuration; add interactive tree resize, stacked
tabs, per-output workspace configuration or a tiling UI. This PR is one
implementable step toward COSMIC-style auto-tiling, **not the completed feature**.

### Acceptance sequence

1. Prove the new Rust tests and all existing Linux/SFTP/QEMU CI on the
   **exact final PR head**; never merge red or queued CI.
2. Bind a per-workspace layout model to **native** managed XDG windows
   and update it on create, move, minimize, restore, destroy, hotplug and
   panel exclusive-zone changes. Compute plans before applying changes.
3. Apply placement through XDG configure and await matching ACK/buffer
   commit; do not acknowledge GPU/KMS presentation speculatively.
4. Implement on/off shortcut, floating toggle, layout orientation, swap,
   interactive split resizing, stack tabs and settings persistence.
5. Exercise multiple physical outputs and VT/suspend/restart scenarios;
   continue the P0 checks in #24 before enabling native login.

## Evidence required for any green check mark

1. Commit SHA and links to **completed/success** Linux build,
   GVfs SFTP, isolated QEMU Wayland/KMS tests.
2. Real client-protocol test (not only a pure policy test) for compositor
   features such as focus, layout, clipboard, screencast.
3. Separate dedicated physical hardware test where #24 requires it,
   with GPU, outputs, recovery behavior and artifacts identified.
4. Fallback labwc stays available; NATIVE_SESSION_READY=false
   until every P0 native session acceptance gate is met.

## Sources

- [COSMIC official product overview](https://system76.com/cosmic)
- [COSMIC Comp source](https://github.com/pop-os/cosmic-comp)
- [COSMIC 2026 7-month release features](https://blog.system76.com/blog/post/cosmic-de-first-seven-months)
- [System76 COSMIC keyboard shortcut reference](https://support.system76.com/support/pop-cosmic-keyboard-shortcuts)
- [COSMIC Epoch 2/3 roadmap](https://system76.com/blog/post/cosmic-epoch-2-and-3-roadmap)
