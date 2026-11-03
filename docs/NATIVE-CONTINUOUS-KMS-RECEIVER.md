# Stage 45: isolated QEMU DRM/KMS continuous receiver

This stage adds qemu-rust-kms-continuous, a second Rust KMS receiver.
Unlike the existing 1–6 frame recipient, it understands MLF1/MLA1 with
64-bit epoch, ticket sequence and scene generation. It intentionally keeps
the original implementation untouched.

The binary is gated by QEMU DMI, dedicated /dev/tty1, absence of display
server environment variables and the opt-in MELANIA_QEMU_CONTINUOUS_KMS.
It uses a different socket from the six-frame route.

The sacrificial guest test may select 8 to 64 presentations; it defaults
to 12. This bound is an EXIT CONDITION FOR THE VM TEST, not an ordinal
size restriction in the protocol or the ContinuousRedraw planner.

For each frame the receiver checks the magic, exact pixel dimensions,
monotonic identity and distinct pixels, writes a DRM framebuffer, then
confirms the CRTC FB_ID. Only then does it emit the matching MLA1 ACK.
It rotates two preallocated framebuffer objects and must restore the
old CRTC and verify its state before reporting success.

This commit alone does not switch the Smithay sender to the new socket,
and cannot claim successful real 12-frame QEMU presentations until a new
guest scenario has been executed. Real GPU login remains disabled and the
Melania interface remains unchanged.

Next: wire the sender to this opt-in receiver, exercise 12 real wl_shm
commits, prove all 12 KMS and Wayland ACKs, and add QMP pixel evidence.
