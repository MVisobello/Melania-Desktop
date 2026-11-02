#!/usr/bin/env python3
"""Stage34: Smithay must not block on KMS I/O and callbacks follow real ACKs."""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
WORKER=(ROOT/"melania-native/src/async_kms_stream.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeAsyncKmsStage34(unittest.TestCase):
    def test_worker_bounded_and_socket_io_off_event_loop(self):
        self.assertIn("pub mod async_kms_stream;",LIB)
        for needle in ("mpsc::sync_channel::<FrameJob>(1)",
                       "mpsc::sync_channel::<FrameResult>(1)",
                       "thread::spawn(move ||",
                       "stream.write_all(&job.pixels)",
                       "stream.read_exact(&mut ack)",
                       "self.sender.try_send(job)",
                       "self.receiver.try_recv()",
                       "mismatched_ack_cannot_be_reported_as_success",
                       "event_loop_can_poll_while_kms_has_not_acknowledged"):
            self.assertIn(needle,WORKER)
        self.assertNotIn("thread::sleep(",BRIDGE)

    def test_no_release_or_callback_until_worker_completion_and_ticket_validation(self):
        commit=BRIDGE.index("fn commit(&mut self,surface:&WlSurface)")
        submit=BRIDGE.index("worker.submit(FrameJob{ticket,ordinal:next,pixels:frame})",commit)
        pending=BRIDGE.index("self.async_pending=Some(PendingAsyncFrame",submit)
        early_return=BRIDGE.index("return;",pending)
        sync_fallback=BRIDGE.index("let handoff:Result<(),Box<dyn Error>>",early_return)
        self.assertLess(submit,pending)
        self.assertLess(pending,early_return)
        self.assertLess(early_return,sync_fallback)
        poll=BRIDGE.index("fn poll_async_kms(&mut self)")
        ack=BRIDGE.index("if !self.render_scheduler.complete_present(pending.ticket)",poll)
        callback=BRIDGE.index("complete_presented_frame(",ack)
        release=BRIDGE.index("pending.buffer.release();",callback)
        self.assertLess(ack,callback)
        self.assertLess(callback,release)
        self.assertIn("state.poll_async_kms()?",BRIDGE)
        self.assertIn("MELANIA_NATIVE_ASYNC_FRAME_COMPLETED={}",BRIDGE)

    def test_guest_and_host_require_actual_async_vm_capture(self):
        for needle in ("MELANIA_NATIVE_ASYNC_KMS=YES_QEMU_VM",
                       "MELANIA_NATIVE_ASYNC_FRAME_SUBMITTED=$index",
                       "MELANIA_NATIVE_ASYNC_FRAME_COMPLETED=$index",
                       "MELANIA_QEMU_NONBLOCKING_KMS_WORKER_PASS=true"):
            self.assertIn(needle,GUEST)
        self.assertIn("MELANIA_QEMU_NONBLOCKING_KMS_WORKER_PASS=true",HOST)
        self.assertIn("test_native_async_kms_stage34.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        for script in ("scripts/run-qemu-melania-native.sh",
                       "scripts/melania-qemu-native-guest.sh"):
            p=subprocess.run(["bash","-n",str(ROOT/script)],
                capture_output=True,text=True,timeout=6)
            self.assertEqual(p.returncode,0,p.stderr)

if __name__=="__main__":
    unittest.main()
