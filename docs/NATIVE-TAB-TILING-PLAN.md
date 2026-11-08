# COSMIC group-aware auto-tiling — model transaction (Stage131)

COSMIC places multiple tabbed windows in **one** tiling slot. This Rust
policy adapter takes the current one-workspace `AutoTiling` and `TabGroups`
states, clones the tiler, excludes hidden group members **on the clone only**,
and returns one placement per active group plus the complete list of
clients whose geometry must eventually match the tiled rectangle.

The adapter preserves all compositor-owned identities and does not accidentally
create a tile for an inactive tab, or mutate floating/minimized/XDG-role
exclusions on the original tiler. It rejects unregistered members,
malformed group references and impossible layouts without mutation.

**Important release boundary:** this is a validated *layout policy*, not
combined native session behavior. `MELANIA_NATIVE_AUTO_TILING=YES_QEMU_VM`
and `MELANIA_NATIVE_TAB_STACKS=YES_QEMU_VM` remain mutually exclusive in
the running QEMU bridge. Before enabling both, add a transactional XDG
configure/ACK/buffer-commit ledger for **every** stack member, ensure hidden
members retain owned buffers without hit-testing or painting, and verify real
QMP CRTC pixels and VT/DRM restoration. Never let a size change expose
old client pixels. Keep `NATIVE_SESSION_READY=false` and labwc as fallback.
