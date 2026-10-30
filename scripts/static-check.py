#!/usr/bin/env python3
"""Conservative structural checks only; NOT equivalent to cargo check or UI testing."""
import pathlib, subprocess, tomllib, xml.etree.ElementTree as ET
import struct, zlib

root = pathlib.Path(__file__).resolve().parent.parent
workspace = tomllib.loads((root/'Cargo.toml').read_text())
assert workspace['workspace']['package']['version'] == '0.10.0'
members = workspace['workspace']['members']
assert len(members) == len(set(members))
for member in members:
    manifest = root/member/'Cargo.toml'
    assert manifest.exists(), f'Cargo missing: {member}'
    cargo = tomllib.loads(manifest.read_text())
    assert 'package' in cargo
    assert (root/member/'src').is_dir()
    for dep, value in cargo.get('dependencies',{}).items():
        if isinstance(value, dict) and 'path' in value:
            assert (root/member/value['path']).resolve().is_dir(), (member,dep)
for script in (root/'scripts').glob('*.sh'):
    subprocess.run(['bash','-n',str(script)],check=True)
for svg in (root/'branding').rglob('*.svg'):
    ET.parse(svg)
for png in (root/'branding').rglob('*.png'):
    data = png.read_bytes()
    assert data.startswith(b'\x89PNG\r\n\x1a\n'), f'Not a PNG: {png}'
    offset = 8
    has_ihdr = has_iend = False
    while offset + 12 <= len(data):
        n = struct.unpack('>I', data[offset:offset+4])[0]
        tag = data[offset+4:offset+8]
        assert offset + n + 12 <= len(data), f'Truncated PNG: {png}'
        payload = data[offset+8:offset+8+n]
        crc = struct.unpack('>I', data[offset+8+n:offset+12+n])[0]
        assert zlib.crc32(tag+payload) == crc, f'Bad PNG CRC: {png}'
        if tag == b'IHDR':
            has_ihdr = True
            width, height = struct.unpack('>II', payload[:8])
            assert 0 < width < 5000 and 0 < height < 5000
        if tag == b'IEND':
            has_iend = True
            break
        offset += n + 12
    assert has_ihdr and has_iend, f'Incomplete PNG: {png}' 
for filename in ['docs/DRAG-DROP.md','docs/COMPOSITOR-DND-M2.md','docs/COMPOSITOR-M3.md','docs/COMPOSITOR-M4.md','melania-core/src/interaction.rs','melania-transfer/src/lib.rs','melania-dnd/src/lib.rs','melania-compositor/src/main.rs']:
    assert (root/filename).is_file()
files = (root/'melania-files/src/main.rs').read_text()
assert 'gdk::FileList::from_array' in files and 'gtk::DropTarget' in files
assert 'remove_confirmed_source' in files
assert 'melania-transfer' in members and 'melania-dnd' in members and 'melania-compositor' in members
compositor = (root/'melania-compositor/src/main.rs').read_text()
assert 'DataDeviceState::new' in compositor and 'surface_at' in compositor and 'ListeningSocket::bind_auto' in compositor
assert 'local_uris' in (root/'melania-dnd/src/lib.rs').read_text()
assert 'fn resize_request' in compositor and 'fn move_request' in compositor
assert 'PressRecord' in compositor and 'fn update_interaction' in compositor
assert 'pub mod interaction;' in (root/'melania-core/src/lib.rs').read_text()
assert 'run-melania-v09.sh' in [p.name for p in (root/'scripts').glob('*.sh')]
assert 'melania-panel' in members and 'melania-shell-model' in members
panel=(root/'melania-panel/src/main.rs').read_text()
assert 'init_layer_shell' in panel and 'DropTarget' in panel and 'start_window' in panel
assert 'WlrLayerShellState::new' in compositor and 'delegate_layer_shell!' in compositor
assert 'snap_geometry' in compositor and 'detect_snap' in compositor
model=(root/'melania-shell-model/src/lib.rs').read_text()
assert model.count('#[test]') >= 15
assert 'fn classify_drop' in model and 'fn detect_snap' in model
assert 'melania-control' in members and 'melania-filectl' in members
assert 'move_local' in (root/'melania-filectl/src/main.rs').read_text()
assert 'PendingTransfer' in files and 'gdk::DragAction::COPY);' in files
assert 'gdk::DragAction::COPY | gdk::DragAction::MOVE' not in files, 'DnD MOVE is not tested/safe yet' 
assert 'running_window_strip' in panel and 'WindowCommand::Activate' in panel
assert 'SOCKET_ENV' in compositor and 'fn control' in compositor
control = (root/'melania-control/src/lib.rs').read_text()
assert control.count('#[test]') >= 5
print(f'STATIC CHECK PASS: {len(members)} crate, shell scripts, SVG/PNG, paths')
print('NOT VERIFIED: Rust compilation, GTK runtime, Wayland compositor, drag/drop E2E')
