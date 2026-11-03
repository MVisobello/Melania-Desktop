# Stage 43: auditing real QEMU scene generations against KMS ACKs

This is a narrowly scoped integration of the new `ContinuousRedraw` planner with the actual native `wayland_bridge` event loop. **It is still a disposable QEMU test, not a continuous real-GPU renderer or a login-ready session.**

The six bounded stages continue to use their established KMS framebuffer writer and their `RenderScheduler` tickets. A second ticket ledger now follows the same scene through `ContinuousRedraw`:

1. When Smithay imports a client-owned `wl_shm` buffer, moves its window, removes a destroyed toplevel or maps the newly created window, `note_scene_change()` observes `MultiWindowScene::layout_generation()`.
2. Immediately before handing a composed frame to the asynchronous KMS worker, `begin_generation_proof(generation)` verifies that the scene is dirty, there is no outstanding generation ticket and the image corresponds to its latest recorded generation.
3. The actual KMS worker confirms the framebuffer before the normal scheduler calls `complete_present(ticket)`.
4. **Only then** `confirm_generation_proof(generation)` accepts the planned generation. Only after both checks may the client receive a `wl_surface.frame.done` or `wl_buffer.release`.
5. On failed worker submissions and rejected presentations, the generation ticket is invalidated. Seat pause/resume invalidates both ledgers.

The disposable QEMU guest requires six *strictly increasing* `MELANIA_NATIVE_SCENE_GENERATION_ACK` records in the live Smithay server log. These cannot be produced by a static Python mock: the bridge writes them only after the real KMS ACK.

## Deliberate limits

- This is an **audit bridge**, not yet the authoritative frame scheduler; the current bounded QEMU stream still stops after six frames and restores the original CRTC.
- The next work item is to migrate KMS scheduling from fixed ordinals to the generation ledger and to test many client commits plus teardown/reopen in a separate disposable guest.
- Leave host GPU access, screen unlocking, X11 compatibility, login/session integration and final desktop readiness unchanged. `NATIVE_SESSION_READY=false` is still mandatory.
- No changes to panel, files, titlebar buttons, icons or themes.
