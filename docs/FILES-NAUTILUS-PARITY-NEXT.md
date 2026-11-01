# Melania Files — Nautilus parity: execution and non-blocking search (2026-11-01)

This milestone retains the existing Melania UI and uses Nautilus as a *functional* reference, not a copied GTK shell. See also `docs/NAUTILUS-COMPATIBILITY-037.md`.

## Added on this branch

- Numbered batch renaming for 2–500 selected files, with a GTK preview showing the old and new names and extension preservation.
- Revalidation when the user confirms and again in the filesystem worker. No writes occur while generating the preview.
- Atomic Linux `renameat2(RENAME_NOREPLACE)` via the existing `melania-transfer::rename_local`. Neither existing targets nor broken symlinks are replaced. On a partial failure prior moves are reversed; if rollback itself fails the paths are shown prominently rather than falsely reporting completion.
- Rust regression tests for successful renaming, injected partial failure/rollback, late destination collision, broken symlink collision and failed rollback.
- Recursive search now runs outside the GTK main loop. Closing the results window sets a cancellation flag checked before each directory and entry; directory/result bounds remain in place.
- MIME-aware Open With dialog populated from real GIO desktop handlers. Explicit per-app `Predefinita` action updates Linux user MIME associations; errors are shown in the window.
- Explicit one-way import of existing local `gtk-3.0`/`gtk-4.0` Nautilus bookmarks. Does not mutate Nautilus bookmarks; remote URI bookmarks are reported as skipped.

## Existing features (main)

Local tabs, two views, multi-selection Ctrl/Shift/Ctrl+A, file transfer with background copy and progress, local move with no overwrite, GIO Trash, metadata read-only, templates, bounded search, favorites, directory monitors and volume listing, real nested Wayland/GTK E2E.

## Not equivalent to all Nautilus functionality yet

- A `gio::File` URI-based browser model and GVfs backends for SMB, SFTP, WebDAV and remote search/operations.
- Mount/unmount/eject flows, authentication, volume management and in-app Trash recovery.
- Tree view, richer list customization, per-view preferences, file permission editor and ACLs. The new Open With picker and MIME defaults need real-world desktop coverage.
- Thumbnails for PDF/video, remote thumbnail policy, fast indexed/content search and star metadata.
- Extensible scripts/plugins, archive integrations, robust undo/redo transactions, resumable operations, drag-and-drop MOVE across applications, XWayland parity, complete Nautilus accessibility/HiDPI.
- Real-world Linux/Wayland testing of this specific branch beyond GitHub Actions.

**Release gate:** Do not claim Nautilus feature parity or merge before Rust/GTK CI and a nested Wayland smoke test pass. Any failures need a follow-up correction.
