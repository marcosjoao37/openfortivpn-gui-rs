use std::collections::VecDeque;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum InstEvent {
    Show,
}

/// Queue of events from the single-instance socket listener. Both the active
/// window session and the headless tray loop drain it (never concurrently:
/// the main thread runs either one or the other).
pub type InstQueue = Arc<Mutex<VecDeque<InstEvent>>>;

pub struct Primary {
    pub events: InstQueue,
    _handle: std::thread::JoinHandle<()>,
}

/// Become the primary instance or notify the existing one.
///
/// `Some(primary)` → this process owns the socket.
/// `None` → the "show" request was delivered to the running instance.
///
/// Stale-socket handling: if binding fails AND no live listener answers the
/// probe, the socket file is removed and the bind is retried once.
pub fn acquire(sock: &Path) -> Option<Primary> {
    if let Some(p) = try_bind(sock) {
        return Some(p);
    }
    if notify(sock) {
        return None;
    }
    let _ = std::fs::remove_file(sock); // stale: probe got ECONNREFUSED
    try_bind(sock)
}

fn try_bind(sock: &Path) -> Option<Primary> {
    let listener = UnixListener::bind(sock).ok()?;
    let events: InstQueue = Arc::new(Mutex::new(VecDeque::new()));
    let queue = Arc::clone(&events);
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 16];
            if let Ok(n) = s.read(&mut buf) {
                if n > 0 && buf.starts_with(b"show") {
                    if let Ok(mut q) = queue.lock() {
                        q.push_back(InstEvent::Show);
                    }
                }
            }
        }
    });
    Some(Primary {
        events,
        _handle: handle,
    })
}

/// Ask the running instance to show its window.
pub fn notify(sock: &Path) -> bool {
    UnixStream::connect(sock)
        .and_then(|mut s| s.write_all(b"show"))
        .is_ok()
}

/// Drain all pending instance events.
pub fn drain(queue: &InstQueue) -> Vec<InstEvent> {
    match queue.lock() {
        Ok(mut q) => q.drain(..).collect(),
        Err(_) => Vec::new(),
    }
}
