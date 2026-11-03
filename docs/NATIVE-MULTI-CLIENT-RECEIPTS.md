# Stage 50: bounded per-surface frame receipts

The experimental Rust Smithay/DRM bridge previously used a single
continuous_latest_client receipt: if two Wayland clients submitted a buffer
while an older GPU frame was awaiting its ACK, only the last submitted
client's buffer and eventual callback were tracked.

This change introduces ClientCommitLedger in melania-native. The compositor
tracks at most two unsubmitted (surface, wl_buffer) receipts, one per
currently supported QEMU scene slot. Repeated commits from the same surface
replace its older copied-buffer receipt and return the old wl_buffer for
release; client pixel bytes are already owned by MultiWindowScene.

After successful KMS worker submission, the pending receipts are moved
atomically into the submitted batch. On the matching, verified CRTC ACK
the bridge:

1. Releases each submitted wl_buffer, never on an unverified result.
2. Sends a surface callback if that surface is still live and has no
   newer pending wl_shm commit.
3. Allows callbacks of unchanged surface B even if A changed while its
   earlier framebuffer was pending. New A callbacks wait for the next ACK.

The implementation is bounded and independent of any fixed frame ordinal.
It keeps the VM-only safety opt-ins; no native login or host GPU mode-set
is enabled, and the approved Melania UI is untouched.

Next: test simultaneous client updates during an actual QEMU page flip,
then integrate a general-purpose dynamic Wayland window tree, source
damage regions, GPU hotplug/VT recovery and a GBM/DRM native backend.
