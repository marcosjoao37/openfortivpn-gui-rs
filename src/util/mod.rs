pub mod single_instance;

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// XDG-backed app paths.
#[derive(Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub book_path: PathBuf,
    pub sock: PathBuf,
}

pub fn paths() -> Paths {
    let base = dirs::config_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let config_dir = base.join("openfortivpn-gui");
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let book_path = config_dir.join("profiles.json");
    Paths {
        config_dir,
        book_path,
        sock: runtime.join("openfortivpn-gui.sock"),
    }
}

/// Minimal `which`: resolve `name` against PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// PIDs whose /proc/<pid>/comm matches `comm_name`.
pub fn find_pids(comm_name: &str) -> Vec<u32> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for entry in rd.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) {
                if comm.trim() == comm_name {
                    if let Ok(pid) = name.parse::<u32>() {
                        out.push(pid);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out
}

pub fn net_ifaces() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/class/net") {
        for entry in rd.flatten() {
            if let Some(n) = entry.file_name().to_str() {
                out.push(n.to_owned());
            }
        }
    }
    out.sort();
    out
}

/// Candidate VPN interfaces: ppp* / tun* (openfortivpn creates ppp).
pub fn vpn_ifaces() -> Vec<String> {
    net_ifaces()
        .into_iter()
        .filter(|n| n.starts_with("ppp") || n.starts_with("tun"))
        .collect()
}

/// (rx_bytes, tx_bytes) from /sys, if the interface exists.
pub fn iface_bytes(iface: &str) -> Option<(u64, u64)> {
    let read = |what: &str| -> Option<u64> {
        std::fs::read_to_string(format!("/sys/class/net/{iface}/statistics/{what}"))
            .ok()?
            .trim()
            .parse()
            .ok()
    };
    Some((read("rx_bytes")?, read("tx_bytes")?))
}

pub fn username() -> String {
    std::env::var("USER").unwrap_or_else(|_| whoami_fallback())
}

fn whoami_fallback() -> String {
    std::process::Command::new("id")
        .arg("-un")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "user".into())
}

/// Decode a PNG into egui/eframe window-icon data.
pub fn icon_data(png: &[u8]) -> Result<egui::IconData, String> {
    let img = image::load_from_memory(png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok(egui::IconData {
        width: w,
        height: h,
        rgba: img.into_raw(),
    })
}

/// Decode a PNG into raw RGBA bytes for tray icons.
pub fn png_rgba(png: &[u8]) -> Result<(Arc<Vec<u8>>, u32, u32), String> {
    let img = image::load_from_memory(png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok((Arc::new(img.into_raw()), w, h))
}

/// Touch a file's permissions (best effort) — used for 0600 secrets.
pub fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

/// Fire a desktop notification via org.freedesktop.Notifications (DBus).
/// Best effort: failures are returned, never fatal.
pub fn notify(summary: &str, body: &str) -> Result<(), String> {
    notify_rust::Notification::new()
        .summary(summary)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(6000))
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
