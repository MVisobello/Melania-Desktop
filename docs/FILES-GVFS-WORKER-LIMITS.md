# Melania Files — bounded GVfs worker lifecycle

The network file browser now has **at most one active directory-enumeration worker per tab**. Rapid search, sort or navigation actions cancel older requests and replace a single pending request. Only the newest generation may render GTK rows; the next scan starts after the previous one returns.

The GIO backend checks cancellation *before* invoking any provider enumeration. It still cannot guarantee immediate cancellation of a provider blocked inside a synchronous GIO call; on some slow/unavailable servers, the latest scan must wait for the active call to return. Fully asynchronous GIO provider cancellation remains future work.

On tab teardown, monitors and pending requests are cancelled/cleared. This improves resource use under rapid typing without changing data operations or authentication. Rust/Python and nested Wayland tests verify the code/GUI, but this **does not substitute real SMB/SFTP/WebDAV stress testing**. Track broader parity in issue #78.
