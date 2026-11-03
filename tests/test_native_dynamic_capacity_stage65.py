from pathlib import Path
import unittest
S=(Path(__file__).resolve().parents[1]/'melania-native/src/wayland_bridge.rs').read_text()
class DynamicCapacity(unittest.TestCase):
    def test_only_dynamic_qemu_scene_has_sixteen_slots(self):
        self.assertIn('ClientCommitLedger::with_capacity(16)',S)
        self.assertIn('WindowSlots::with_capacity(16)',S)
        self.assertIn('} else {ClientCommitLedger::new()}',S)
        self.assertIn('} else {WindowSlots::default()}',S)
        self.assertIn('YES_QEMU_VM',S)
if __name__=='__main__':unittest.main()
