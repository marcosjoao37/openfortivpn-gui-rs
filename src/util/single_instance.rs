use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

pub enum InstEvent {
    Show,
}

pub struct Primary {
    rx: Receiver<InstEvent>,
    _handle: std::thread::JoinHandle<()>,
}

impl Primary {
    pub fn into_events(self) -> Receiver<InstEvent> {
        self.rx
    }
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
    let (tx, rx) = channel::<InstEvent>();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 16];
            if let Ok(n) = s.read(&mut buf) {
                if n > 0 && buf.starts_with(b"show") {
                    let _ = tx.send(InstEvent::Show);
                }
            }
        }
    });
    Some(Primary {
        rx,
        _handle: handle,
    })
}

/// Ask the running instance to show its window.
fn notify(sock: &Path) -> bool {
    UnixStream::connect(sock)
        .and_then(|mut s| s.write_all(b"show"))
        .is_ok()
}
