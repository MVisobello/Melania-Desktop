#!/usr/bin/env python3
"""Stage32: per-frame ACK must match a valid, active scheduler ticket.

Structural checks accompany real Rust scheduler unit tests and QEMU's
end-to-end Wayland wl_shm->Rust KMS->wl_callback.done verification.
"""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCHED=(ROOT/"melania-native/src/render_scheduler.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeRenderSchedulerStage32(unittest.TestCase):
    def test_scheduler_guards_and_unit_tests(self):
        self.assertIn("pub mod render_scheduler;",LIB)
        for fragment in (
            "pub struct FrameTicket",
            "pub struct RenderScheduler",
            "self.pending.is_some()",
            "self.dirty = true",
            "self.pending != Some(ticket)",
            "self.epoch = self.epoch.wrapping_add(1)",
            "self.pending = None",
            "fn coalesces_damage_while_a_frame_is_outstanding()",
            "fn cannot_acknowledge_duplicate_wrong_or_old_generation()",
            "fn failed_presentation_requeues_but_never_completes()",
            "fn hotplug_revokes_pending_and_drops_queued_damage()",
        ):
            self.assertIn(fragment,SCHED)

    def test_real_wayland_callbacks_only_after_verified_ticket_and_ack(self):
        self.assertIn("render_scheduler:RenderScheduler",BRIDGE)
        self.assertIn("self.render_scheduler.damage()",BRIDGE)
        self.assertIn("self.render_scheduler.begin_frame()",BRIDGE)
        self.assertIn("self.render_scheduler.complete_present(ticket)",BRIDGE)
        self.assertIn("self.render_scheduler.fail_present(ticket)",BRIDGE)
        self.assertIn("state.render_scheduler.set_seat_active(false)",BRIDGE)
        self.assertIn("state.render_scheduler.set_seat_active(active)",BRIDGE)
        ack=BRIDGE.index("stream.read_exact(&mut ack)")
        completion=BRIDGE.index("self.render_scheduler.complete_present(ticket)")
        callback=BRIDGE.index("let count=complete_presented_frame(",completion)
        release=BRIDGE.index("buffer.release();",callback)
        self.assertLess(ack,completion)
        self.assertLess(completion,callback)
        self.assertLess(callback,release)
        self.assertIn("MELANIA_NATIVE_SCHEDULED_FRAME_ACK={next}",BRIDGE)

    def test_vm_requires_three_distinct_scheduled_presentations(self):
        self.assertIn('for index in 1 2 3; do',GUEST)
        self.assertIn('MELANIA_NATIVE_SCHEDULED_FRAME_ACK=$index',GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_SCHEDULED_FRAMES_PASS=true",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertIn("test_native_render_scheduler_stage32.py",CI)
        run=subprocess.run(["bash","-n",str(ROOT/"scripts/melania-qemu-native-guest.sh")],
            capture_output=True,text=True,timeout=5)
        self.assertEqual(run.returncode,0,run.stderr)

if __name__=="__main__":
    unittest.main()
