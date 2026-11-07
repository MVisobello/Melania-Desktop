"""Stage132: Smithay 0.7 native group resize must use actual XDG ACK and owned buffers."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
LEDGER=(ROOT/"melania-native/src/tab_resize_transaction.rs").read_text()
WORKFLOW=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeTabResizeWiringStage132(unittest.TestCase):
    def test_real_smithay_xdg_toplevel_ack_is_the_only_ack_source(self):
        self.assertIn("XdgPopupSurfaceData,Configure",BRIDGE)
        self.assertIn("fn ack_configure(&mut self,surface:WlSurface,configure:Configure)",BRIDGE)
        self.assertIn("Configure::Toplevel(configure)",BRIDGE)
        self.assertIn("let serial:u32=configure.serial.into();",BRIDGE)
        self.assertIn("self.tab_resize_transaction.ack_configure(epoch,id,serial)",BRIDGE)
        self.assertNotIn("self.tab_resize_transaction.sent_configure(",BRIDGE)
    def test_only_owned_imported_shm_commit_can_count(self):
        # If a future refactor moves this block, fail rather than silently
        # accepting a configure as client-owned pixels.
        self.assertIn("self.tab_resize_transaction.can_accept_owned_buffer(",BRIDGE)
        guard=BRIDGE.index("self.tab_resize_transaction.can_accept_owned_buffer(")
        insert=BRIDGE.index("self.dynamic_scene.upsert(key,x,y,pixels)")
        marker=BRIDGE.index("self.tab_resize_transaction.owned_buffer_committed(")
        self.assertLess(guard,insert)
        self.assertGreater(marker,insert)
        self.assertIn("MELANIA_NATIVE_TAB_RESIZE_PREIMPORT_REJECTED=",BRIDGE)
        self.assertIn("if let Err(error)=self.tab_resize_transaction.can_accept_owned_buffer(",BRIDGE)
        self.assertIn("buffer.release();",BRIDGE[guard:insert])
        self.assertIn("MELANIA_NATIVE_TAB_RESIZE_BUFFER_REFUSED=",BRIDGE)
        self.assertIn("MELANIA_NATIVE_TAB_RESIZE_BUFFERS_READY=",BRIDGE)
    def test_closure_and_seat_device_loss_abort_all_uncommitted_groups(self):
        self.assertIn("self.tab_resize_transaction.abort_if_member_destroyed(id)",BRIDGE)
        self.assertGreaterEqual(BRIDGE.count("state.tab_resize_transaction.revoke();"),2)
        for member in ("ClientReceipt","MissingConfigure","WrongEpoch",
                       "BufferBeforeAck","BufferSizeMismatch","AlreadyPending"):
            self.assertIn(member,LEDGER)
    def test_never_enable_tiling_with_stacks_before_atomic_apply(self):
        self.assertIn('std::env::var("MELANIA_NATIVE_AUTO_TILING")',BRIDGE)
        self.assertIn('.as_deref()!=Ok("YES_QEMU_VM")',BRIDGE)
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)
        self.assertIn("test_native_tab_resize_wiring_stage132.py",WORKFLOW)

if __name__=="__main__":
    unittest.main()
