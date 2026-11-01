# Melania Files — rich GVfs view and metadata (1 Nov 2026)

This change adds Nautilus-like file browsing capabilities to **existing network tabs** without replacing the Melania theme and tabs.

- **Name / last modification / size / MIME type** sorting on GIO metadata with folders always first; optional descending order and filtering.
- **Hidden** toggle and Ctrl+H for GVfs listings, matching the local view's behavior. No symbolic-link traversal for metadata queries.
- **Fuller list rows** including icon, name, size, MIME type and last modification.
- **Properties** opens a native GTK window and asynchronously queries one item's GIO metadata, including read/write/execute capabilities where supplied by the provider. Does not recursively enumerate folders or claim numeric folder size.
- **GIO directory monitor** attempts to subscribe once after successful listing, debounces provider events and refreshes. Backends without monitor support retain F5 manual refresh. File monitors are cancelled on location changes and tab teardown.
- **No shell command execution, no overwrites, no new permissions**. No authentication from the scan itself.

## Limits

Providers may omit timestamps, MIME values or access rights: treat missing metadata as unavailable rather than making claims. Remote monitors depend on GVfs backend support and can be unavailable. This is tested with Rust and a real local GIO file URI nested Wayland GUI, **not** with real SMB/SFTP/WebDAV servers. Full Nautilus parity remains in issue #78.
