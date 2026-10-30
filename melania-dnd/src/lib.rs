//! State machine for compositor DnD routing. This is a protocol-independent
//! model: it doesn't register Wayland data devices or handle XWayland yet.
//! A compositor adapter must map Smithay's per-seat DnD events to these calls.
use std::collections::HashMap;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Seat(pub u64);
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Surface(pub u64);
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Action { Copy, Move, Link }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Started { seat: Seat, source: Surface },
    Entered { seat: Seat, target: Surface },
    Left { seat: Seat, target: Surface },
    Accepted { seat: Seat, mime: String, action: Action },
    Dropped { seat: Seat, target: Surface, action: Action },
    Cancelled { seat: Seat },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Busy, NotActive, InvalidSource, NotEntered, UnsupportedMime, UnsupportedAction,
}
#[derive(Debug, Clone)]
struct Drag {
    source: Surface,
    target: Option<Surface>,
    mimes: Vec<String>,
    actions: Vec<Action>,
    chosen: Option<(String, Action)>,
}
#[derive(Default)]
pub struct Router { active: HashMap<Seat, Drag> }
impl Router {
    pub fn new() -> Self { Self::default() }
    pub fn active_count(&self) -> usize { self.active.len() }
    pub fn start(&mut self, seat: Seat, source: Surface, mimes: Vec<String>, actions: Vec<Action>) -> Result<Event, Error> {
        if self.active.contains_key(&seat) { return Err(Error::Busy); }
        if mimes.is_empty() || actions.is_empty() { return Err(Error::UnsupportedMime); }
        self.active.insert(seat, Drag { source, target: None, mimes, actions, chosen: None });
        Ok(Event::Started { seat, source })
    }
    pub fn enter(&mut self, seat: Seat, target: Surface) -> Result<Vec<Event>, Error> {
        let drag = self.active.get_mut(&seat).ok_or(Error::NotActive)?;
        let mut events = vec![];
        if drag.target == Some(target) { return Ok(events); }
        if let Some(old) = drag.target.take() { events.push(Event::Left { seat, target: old }); }
        drag.chosen = None;
        drag.target = Some(target);
        events.push(Event::Entered { seat, target });
        Ok(events)
    }
    pub fn leave(&mut self, seat: Seat) -> Result<Option<Event>, Error> {
        let drag = self.active.get_mut(&seat).ok_or(Error::NotActive)?;
        drag.chosen = None;
        Ok(drag.target.take().map(|target| Event::Left { seat, target }))
    }
    pub fn accept(&mut self, seat: Seat, mime: &str, action: Action) -> Result<Event, Error> {
        let drag = self.active.get_mut(&seat).ok_or(Error::NotActive)?;
        if drag.target.is_none() { return Err(Error::NotEntered); }
        if !drag.mimes.iter().any(|m| m == mime) { return Err(Error::UnsupportedMime); }
        if !drag.actions.contains(&action) { return Err(Error::UnsupportedAction); }
        drag.chosen = Some((mime.to_owned(), action));
        Ok(Event::Accepted { seat, mime: mime.to_owned(), action })
    }
    pub fn finish_drop(&mut self, seat: Seat) -> Result<Event, Error> {
        let drag = self.active.get(&seat).ok_or(Error::NotActive)?;
        let target = drag.target.ok_or(Error::NotEntered)?;
        let (_, action) = drag.chosen.as_ref().ok_or(Error::UnsupportedAction)?;
        let event = Event::Dropped { seat, target, action: *action };
        self.active.remove(&seat);
        Ok(event)
    }
    pub fn cancel(&mut self, seat: Seat) -> Result<Event, Error> {
        if self.active.remove(&seat).is_none() { return Err(Error::NotActive); }
        Ok(Event::Cancelled { seat })
    }
    pub fn surface_destroyed(&mut self, surface: Surface) -> Vec<Event> {
        let mut cancelled = Vec::new();
        for (seat, drag) in &mut self.active {
            if drag.source == surface { cancelled.push(*seat); }
            else if drag.target == Some(surface) { drag.target = None; drag.chosen = None; }
        }
        cancelled.into_iter().filter_map(|seat| self.cancel(seat).ok()).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn start(r: &mut Router, seat: Seat) {
        r.start(seat,Surface(1),vec!["text/uri-list".into()],vec![Action::Copy,Action::Move]).unwrap();
    }
    #[test] fn transfer_negotiates_and_completes() {
        let mut r=Router::new();start(&mut r,Seat(0));
        assert_eq!(r.enter(Seat(0),Surface(2)).unwrap(),vec![Event::Entered{seat:Seat(0),target:Surface(2)}]);
        r.accept(Seat(0),"text/uri-list",Action::Copy).unwrap();
        assert_eq!(r.finish_drop(Seat(0)).unwrap(),Event::Dropped{seat:Seat(0),target:Surface(2),action:Action::Copy});
        assert_eq!(r.active_count(),0);
    }
    #[test] fn cannot_drop_unaccepted_payload() {
        let mut r=Router::new();start(&mut r,Seat(0));r.enter(Seat(0),Surface(2)).unwrap();
        assert_eq!(r.finish_drop(Seat(0)),Err(Error::UnsupportedAction));
    }
    #[test] fn cannot_accept_unoffered_mime_or_action() {
        let mut r=Router::new();start(&mut r,Seat(0));r.enter(Seat(0),Surface(2)).unwrap();
        assert_eq!(r.accept(Seat(0),"text/plain",Action::Copy),Err(Error::UnsupportedMime));
        assert_eq!(r.accept(Seat(0),"text/uri-list",Action::Link),Err(Error::UnsupportedAction));
    }
    #[test] fn leaving_clears_acceptance_and_cancel_clears_session() {
        let mut r=Router::new();start(&mut r,Seat(0));r.enter(Seat(0),Surface(2)).unwrap();
        r.accept(Seat(0),"text/uri-list",Action::Move).unwrap();r.leave(Seat(0)).unwrap();
        r.enter(Seat(0),Surface(3)).unwrap();
        assert_eq!(r.finish_drop(Seat(0)),Err(Error::UnsupportedAction));
        assert_eq!(r.cancel(Seat(0)),Ok(Event::Cancelled{seat:Seat(0)}));
    }
    #[test] fn multiple_seats_isolated_and_destroy_source_cancelled() {
        let mut r=Router::new();start(&mut r,Seat(0));start(&mut r,Seat(1));
        assert_eq!(r.start(Seat(0),Surface(3),vec!["x".into()],vec![Action::Copy]),Err(Error::Busy));
        let events=r.surface_destroyed(Surface(1));
        assert_eq!(events.len(),2);assert_eq!(r.active_count(),0);
    }
    #[test] fn destroying_target_invalidates_drop_without_cancelling_source() {
        let mut r=Router::new();start(&mut r,Seat(0));r.enter(Seat(0),Surface(2)).unwrap();
        r.accept(Seat(0),"text/uri-list",Action::Copy).unwrap();
        assert!(r.surface_destroyed(Surface(2)).is_empty());
        assert_eq!(r.finish_drop(Seat(0)),Err(Error::NotEntered));
    }
}

/// A strict subset of `text/uri-list` for drops into Melania Files.
/// Clipboard/DnD payloads are untrusted bytes: accept only local file URIs.
/// This parser is independent of GTK and can be fuzz-tested without a display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriListError {
    TooLarge,
    TooManyItems,
    InvalidUtf8,
    UnsupportedUri,
    InvalidEscape,
    EmbeddedNul,
    Empty,
}

/// Converts percent-encoded file URIs to local UNIX paths.
/// Ignores blank lines and RFC-style comment lines beginning with `#`.
#[cfg(unix)]
pub fn local_uris(data: &[u8]) -> Result<Vec<std::path::PathBuf>, UriListError> {
    use std::{collections::HashSet, os::unix::ffi::OsStringExt};
    const MAX_BYTES: usize = 1024 * 1024;
    const MAX_ITEMS: usize = 256;
    if data.len() > MAX_BYTES { return Err(UriListError::TooLarge); }
    let input = std::str::from_utf8(data).map_err(|_| UriListError::InvalidUtf8)?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut offered_count = 0usize;
    for raw_line in input.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') { continue; }
        offered_count += 1;
        if offered_count > MAX_ITEMS { return Err(UriListError::TooManyItems); }
        let raw_path = line.strip_prefix("file:///")
            .map(|rest| format!("/{rest}"))
            .or_else(|| line.strip_prefix("file://localhost/")
                .map(|rest| format!("/{rest}")))
            .ok_or(UriListError::UnsupportedUri)?;
        let bytes = raw_path.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut n = 0;
        while n < bytes.len() {
            if bytes[n] == b'%' {
                if n + 2 >= bytes.len() { return Err(UriListError::InvalidEscape); }
                let digit = |value: u8| -> Option<u8> {
                    match value {
                        b'0'..=b'9' => Some(value - b'0'),
                        b'a'..=b'f' => Some(value - b'a' + 10),
                        b'A'..=b'F' => Some(value - b'A' + 10),
                        _ => None,
                    }
                };
                let high = digit(bytes[n + 1]).ok_or(UriListError::InvalidEscape)?;
                let low = digit(bytes[n + 2]).ok_or(UriListError::InvalidEscape)?;
                decoded.push(high * 16 + low);
                n += 3;
            } else {
                decoded.push(bytes[n]);
                n += 1;
            }
        }
        if decoded.contains(&0) { return Err(UriListError::EmbeddedNul); }
        // Do not expand `..`, `$HOME`, network URLs or shell commands here.
        let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(decoded));
        if !path.is_absolute() { return Err(UriListError::UnsupportedUri); }
        if seen.insert(path.clone()) { result.push(path); }
    }
    if result.is_empty() { return Err(UriListError::Empty); }
    Ok(result)
}

#[cfg(all(test, unix))]
mod uri_tests {
    use super::*;
    #[test]
    fn accepts_local_files_and_removes_duplicates() {
        let data = b"# Drag selection\r\nfile:///home/user/nota%20prova.txt\r\nfile:///home/user/nota%20prova.txt\nfile://localhost/tmp/Photo%20One.png\n";
        let items = local_uris(data).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].to_str(), Some("/home/user/nota prova.txt"));
        assert_eq!(items[1].to_str(), Some("/tmp/Photo One.png"));
    }
    #[test]
    fn rejects_remote_or_dangerous_items() {
        assert_eq!(local_uris(b"https://example.org/evil"), Err(UriListError::UnsupportedUri));
        assert_eq!(local_uris(b"file://remote-host/tmp/a"), Err(UriListError::UnsupportedUri));
        assert_eq!(local_uris(b"file:///tmp/%00x"), Err(UriListError::EmbeddedNul));
        assert_eq!(local_uris(b"file:///tmp/%QZ"), Err(UriListError::InvalidEscape));
        assert_eq!(local_uris(b"# comment only"), Err(UriListError::Empty));
    }
    #[test]
    fn prevents_payload_overload() {
        assert_eq!(local_uris(&vec![b'a'; 1024 * 1024 + 1]), Err(UriListError::TooLarge));
        let data = "file:///tmp/a\n".repeat(257);
        assert_eq!(local_uris(data.as_bytes()), Err(UriListError::TooManyItems));
    }
}
