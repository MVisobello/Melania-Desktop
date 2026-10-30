#!/usr/bin/env python3
"""Static/offline invariants only. NEVER claims Rust was compiled or Wayland was launched."""
from pathlib import Path
import re
import sys
import unittest

ROOT=Path(__file__).resolve().parents[1]

class WorkspaceReleaseChecks(unittest.TestCase):
    def read(self, relative):
        return (ROOT/relative).read_text(encoding='utf-8')
    def test_workspace_crate_registered(self):
        self.assertIn('"melania-workspaces"',self.read('Cargo.toml'))
        self.assertIn('melania-workspaces = { path = "../melania-workspaces" }',self.read('melania-compositor/Cargo.toml'))
    def test_core_has_bounded_1_based_workspace_ids(self):
        s=self.read('melania-workspaces/src/lib.rs')
        for token in ['DEFAULT_WORKSPACES: u8 = 4', 'fn register(', 'fn forget(', 'fn switch(', 'fn send(', 'fn is_visible(', 'fn workspace_of(']:
            self.assertIn(token,s)
        self.assertIn('id == 0 || id > self.count',s)
        self.assertIn('dest == 0 || dest > self.count',s)
    def test_ipc_has_4_commands_bounded(self):
        s=self.read('melania-control/src/lib.rs')
        for token in ['STATUS\\n','WORKSPACE {ws}\\n','SEND {id} {ws}\\n', 'MAX_COMMAND: usize = 64', 'read_command', 'current_workspace(', 'respond_workspace(']:
            self.assertIn(token,s)
        self.assertIn('self.active',self.read('melania-workspaces/src/lib.rs'))
    def test_workspace_management_is_in_compositor(self):
        s=self.read('melania-compositor/src/main.rs')
        for token in ['workspaces.register(id)', 'workspaces.forget(id)', 'fn switch_workspace(', 'fn send_to_workspace(', 'ControlCommand::Workspace(', 'ControlCommand::Send(', 'self.space.unmap_elem', 'self.space.map_element']:
            self.assertIn(token,s)
        self.assertIn('if self.workspaces.is_visible(w.id) && !w.minimized',s)
    def test_taskbar_polls_actual_workspace(self):
        s=self.read('melania-panel/src/main.rs')
        for token in ['fn workspace_strip()', 'current_workspace(path)', 'WindowCommand::Workspace(n)', 'bar.append(&workspace_strip());']:
            self.assertIn(token,s)
        self.assertIn('.melania-workspace-active', self.read('melania-panel/assets/panel.css'))
    def test_popup_geometry_is_not_misrepresented_as_wired(self):
        s=self.read('melania-workspaces/src/popup.rs')
        for tok in ['pub fn constrain(', 'popup.x.clamp(left,right)', 'tiny_output_does_not_underflow']:
            self.assertIn(tok,s)
        self.assertIn('ancora da collegare',self.read('docs/MILESTONE-026.md'))
    def test_shaders_present_separable(self):
        blur=self.read('melania-compositor/shaders/aero-blur-pass.frag')
        comp=self.read('melania-compositor/shaders/aero-composite.frag')
        self.assertIn('#version 300 es',blur)
        self.assertIn('u_texel_step',blur)
        self.assertIn('frag_color',comp)
        self.assertIn('u_tint_strength',comp)
    def test_glass_is_clearly_not_claimed_active(self):
        s=self.read('docs/MILESTONE-026.md')
        self.assertIn('NON ancora integrati',s)
        self.assertIn('sfocatura GPU non è ancora integrata',self.read('README.md'))
    def test_drag_drop_copy_safety_not_changed(self):
        s=self.read('melania-files/src/main.rs')
        self.assertIn('gdk::DragAction::COPY',s)
        self.assertIn('gdk::FileList',s)
    def test_exit_does_not_shutdown_host(self):
        s=self.read('melania-compositor/src/main.rs')
        self.assertIn('exit_requested',s)
        self.assertNotIn('systemctl poweroff',s)

if __name__=='__main__':
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(WorkspaceReleaseChecks)
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(0 if result.wasSuccessful() else 1)
