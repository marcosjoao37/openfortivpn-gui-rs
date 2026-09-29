use crate::controller::sudo::SudoProbe;
use crate::model::profile::Profile;
use crate::model::state::{ConnPhase, Stats};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct ConnectArgs {
    pub profile: Profile,
    pub vpn_pass: Zeroizing<String>,
    pub sudo_pass: Option<Zeroizing<String>>,
    pub mode: SudoProbe,
}

pub enum Cmd {
    Connect(Box<ConnectArgs>),
    Quit,
}

pub enum UiEvent {
    Log(String),
    Phase(ConnPhase),
    Stats { stats: Stats, iface: String },
    IfaceGone,
    CertUnknown(String),
    SudoAuthFailed,
    ConnectFailed(String),
    VpnExited(i32),
}

/// Handle used by the view thread. Secrets flow through the command channel
/// as `Zeroizing<String>` and are never placed on argv or persisted.
pub struct VpnController {
    cmd_tx: Sender<Cmd>,
    pgid: Arc<AtomicI32>,
}

impl VpnController {
    pub fn new(evt_tx: Sender<UiEvent>) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let pgid = Arc::new(AtomicI32::new(0));
        let pg = Arc::clone(&pgid);
        std::thread::spawn(move || manager(cmd_rx, evt_tx, pg));
        Self { cmd_tx, pgid }
    }

    pub fn connect(
        &self,
        profile: Profile,
        vpn_pass: Zeroizing<String>,
        sudo_pass: Option<Zeroizing<String>>,
        mode: SudoProbe,
    ) {
        let _ = self.cmd_tx.send(Cmd::Connect(Box::new(ConnectArgs {
            profile,
            vpn_pass,
            sudo_pass,
            mode,
        })));
    }

    /// SIGTERM the process group (sudo + openfortivpn + pppd), SIGKILL fallback after 5 s.
    pub fn disconnect(&self) {
        let g = self.pgid.load(Ordering::SeqCst);
        if g > 0 {
            kill_group(g, libc::SIGTERM);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                kill_group(g, libc::SIGKILL);
            });
        }
    }

    pub fn kill_external(&self, pid: u32) {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }

    pub fn quit(&self) {
        let _ = self.cmd_tx.send(Cmd::Quit);
    }
}

fn kill_group(pgid: i32, sig: i32) {
    unsafe {
        libc::kill(-pgid, sig);
        libc::kill(pgid, sig); // fallback if setsid failed
    }
}

fn manager(rx: Receiver<Cmd>, evt_tx: Sender<UiEvent>, pgid: Arc<AtomicI32>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Quit => break,
            Cmd::Connect(args) => run_session(
                args.profile,
                args.vpn_pass,
                args.sudo_pass,
                args.mode,
                &evt_tx,
                &pgid,
            ),
        }
    }
}

#[derive(Default)]
struct ScanFlags {
    info_seen: bool,
    cert: Option<String>,
}

fn run_session(
    profile: Profile,
    vpn_pass: Zeroizing<String>,
    sudo_pass: Option<Zeroizing<String>>,
    mode: SudoProbe,
    evt_tx: &Sender<UiEvent>,
    pgid: &Arc<AtomicI32>,
) {
    let Some(bin) = mode.binary() else {
        let _ = evt_tx.send(UiEvent::ConnectFailed(
            "openfortivpn binary is missing.".into(),
        ));
        return;
    };
    let passwordless = mode.passwordless();

    let conf = match write_temp_config(&profile, &vpn_pass) {
        Ok(c) => c,
        Err(e) => {
            let _ = evt_tx.send(UiEvent::ConnectFailed(format!(
                "failed to write temp config: {e}"
            )));
            return;
        }
    };

    let _ = evt_tx.send(UiEvent::Log(format!(
        "Launching: sudo {} --config (temp file, 0600)",
        bin.display()
    )));

    let mut cmd = Command::new("sudo");
    if !passwordless {
        cmd.arg("-S").arg("-p").arg("");
    }
    cmd.arg(bin).arg("--config").arg(conf.path());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Own process group so we can tear down sudo + openfortivpn + pppd together.
    unsafe {
        let _ = cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = evt_tx.send(UiEvent::ConnectFailed(format!(
                "failed to launch sudo: {e}"
            )));
            return;
        }
    };
    pgid.store(child.id() as i32, Ordering::SeqCst);

    // Feed sudo password on stdin (single line), then close stdin.
    if let Some(sudo) = &sudo_pass {
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(sudo.as_bytes());
            let _ = si.write_all(b"\n");
        }
    }

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let tx_out = evt_tx.clone();
    let tx_err = evt_tx.clone();
    let h_out = std::thread::spawn(move || pump(stdout, tx_out));
    let h_err = std::thread::spawn(move || pump(stderr, tx_err));

    let status = child.wait();
    pgid.store(0, Ordering::SeqCst);
    let f_out = h_out.join().unwrap_or_default();
    let f_err = h_err.join().unwrap_or_default();
    let info_seen = f_out.info_seen || f_err.info_seen;

    drop(conf); // remove temp config (0600) as soon as the child is gone

    match status {
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            if code == 0 {
                let _ = evt_tx.send(UiEvent::VpnExited(0));
            } else if !info_seen {
                // openfortivpn never emitted its INFO banner → sudo rejected us.
                let _ = evt_tx.send(UiEvent::SudoAuthFailed);
            } else {
                let _ = evt_tx.send(UiEvent::VpnExited(code));
            }
        }
        Err(e) => {
            let _ = evt_tx.send(UiEvent::ConnectFailed(format!(
                "failed to wait on sudo: {e}"
            )));
        }
    }
}

/// Stream reader → log events + pattern scanning (cert fingerprint, INFO banner).
fn pump<R: std::io::Read>(reader: R, tx: Sender<UiEvent>) -> ScanFlags {
    let mut flags = ScanFlags::default();
    for line in BufReader::new(reader).lines() {
        match line {
            Ok(l) => {
                if l.contains("INFO:") {
                    flags.info_seen = true;
                }
                if flags.cert.is_none() {
                    if let Some(h) = extract_sha256(&l) {
                        flags.cert = Some(h.clone());
                        let _ = tx.send(UiEvent::CertUnknown(h));
                    }
                }
                let _ = tx.send(UiEvent::Log(l));
            }
            Err(_) => break,
        }
    }
    flags
}

/// Find a 64-char hex digest mentioned near "sha256" (locale-tolerant).
pub fn extract_sha256(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let start = lower.find("sha256")?;
    let window = &lower[start..(start + 260).min(lower.len())];
    let chars: Vec<char> = window.chars().collect();
    let mut run = String::new();
    for c in chars {
        if c.is_ascii_hexdigit() {
            run.push(c);
            if run.len() == 64 {
                return Some(run);
            }
        } else {
            run.clear();
            if !c.is_ascii_whitespace() && c != ':' && c != '=' && c != ',' {
                continue;
            }
        }
    }
    None
}

/// Secrets live in a 0600 temp config file passed via `--config`, never on argv.
pub fn temp_config_contents(profile: &Profile, vpn_pass: &str) -> String {
    let (host, port) = profile.host_port();
    let mut s = format!(
        "host = {}\nport = {}\nusername = {}\npassword = {}\n",
        host,
        port,
        profile.username.trim(),
        vpn_pass
    );
    if profile.trust_cert && !profile.trusted_cert.trim().is_empty() {
        s.push_str(&format!(
            "trusted-cert = {}\n",
            profile.trusted_cert.trim().to_lowercase()
        ));
    }
    s
}

pub fn build_args(bin: &str, conf: &Path, passwordless: bool) -> Vec<String> {
    let mut v = vec!["sudo".to_owned()];
    if !passwordless {
        v.push("-S".into());
        v.push("-p".into());
        v.push(String::new());
    }
    v.push(bin.to_owned());
    v.push("--config".into());
    v.push(conf.display().to_string());
    v
}

struct ConfigGuard(PathBuf);

impl ConfigGuard {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_temp_config(profile: &Profile, vpn_pass: &str) -> Result<ConfigGuard, String> {
    let path = std::env::temp_dir().join(format!("ofg-{}.conf", Uuid::new_v4()));
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| e.to_string())?;
    f.write_all(temp_config_contents(profile, vpn_pass).as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(ConfigGuard(path))
}
