pub mod about;
pub mod main_window;
pub mod profile_editor;

use crate::controller::stats;
use crate::controller::sudo::SudoProbe;
use crate::controller::tray::{TrayApp, TrayVariant};
use crate::controller::vpn::{UiEvent, VpnController};
use crate::model::profile::{Profile, ProfileBook};
use crate::model::state::{fmt_kb, fmt_mb, ConnPhase, UiState};
use crate::util;
use crate::util::single_instance::InstEvent;
use egui::ViewportCommand;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::menu::MenuEvent;
use zeroize::Zeroizing;

/// Timestamped stderr debug line (elapsed ms since process start).
pub fn dbg_log(msg: &str) {
    static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    let ms = START.elapsed().as_millis();
    eprintln!("[ofg +{ms}ms] {msg}");
}

/// Runtime state shared between the UI and the tray/menu world: VPN controller
/// (outlives everything — it supervises the openfortivpn child), tray, event
/// queue, persistent log and the single-instance queue.
/// Single-threaded by design (UI + tray all run on the main thread), hence `Rc`.
pub struct SharedRuntime {
    pub paths: util::Paths,
    pub vpn: VpnController,
    pub tray: Option<TrayApp>,
    pub events: crate::controller::EventQueue,
    pub inst_events: util::single_instance::InstQueue,
    pub log: Arc<Mutex<VecDeque<String>>>,
    pub distro_pretty: String,
    pub distro_cmd: String,
}

impl SharedRuntime {
    pub fn push_log(&self, line: String) {
        let mut log = match self.log.lock() {
            Ok(l) => l,
            Err(p) => p.into_inner(),
        };
        log.push_back(line);
        while log.len() > 500 {
            log.pop_front();
        }
    }
}

pub struct PendingConnect {
    pub profile: Profile,
    pub vpn_pass: Zeroizing<String>,
}

pub struct Parts {
    pub shared: Rc<SharedRuntime>,
    pub book: ProfileBook,
    pub probe: SudoProbe,
}

pub struct OFGApp {
    pub shared: Rc<SharedRuntime>,
    pub book: ProfileBook,
    pub state: UiState,
    pub ctx: Option<egui::Context>,
    pub ticker_started: bool,
    pub session_started: Instant,
    pub last_heartbeat: Duration,
    pub smoke_done: bool,
    pub on_wayland: bool,
    pub vpn_pass: String,
    pub sudo_buf: String,
    pub sudo_session: Option<Zeroizing<String>>,
    pub pending_connect: Option<PendingConnect>,
    pub editor: Option<profile_editor::EditorState>,
    pub about_open: bool,
    pub quit_confirm: bool,
    pub quitting: bool,
    pub stats_stop: Arc<AtomicBool>,
    pub connect_hint: Option<String>,
}

impl OFGApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, p: Parts) -> Self {
        let (installed, passwordless, binary_path) = match &p.probe {
            SudoProbe::MissingBinary => (false, false, None),
            SudoProbe::NeedsPassword(b) => (true, false, Some(b.clone())),
            SudoProbe::Passwordless(b) => (true, true, Some(b.clone())),
        };
        let mut state = UiState {
            installed,
            passwordless,
            binary_path,
            ..Default::default()
        };

        // Adopt an openfortivpn session that is already running (external or
        // left over from a previous window session of this very process).
        let stop = Arc::new(AtomicBool::new(false));
        let pids = util::find_pids("openfortivpn");
        let ifaces = util::vpn_ifaces();
        if let (Some(pid), Some(iface)) = (pids.first().copied(), ifaces.first().cloned()) {
            state.phase = ConnPhase::External {
                pid,
                iface: iface.clone(),
            };
            stats::spawn(
                None,
                Some(iface),
                Arc::clone(&p.shared.events),
                Arc::clone(&stop),
            );
        }

        let app = Self {
            shared: p.shared,
            book: p.book,
            state,
            ctx: None,
            ticker_started: false,
            session_started: Instant::now(),
            last_heartbeat: Duration::ZERO,
            smoke_done: false,
            on_wayland: std::env::var_os("WAYLAND_DISPLAY").is_some(),
            vpn_pass: String::new(),
            sudo_buf: String::new(),
            sudo_session: None,
            pending_connect: None,
            editor: None,
            about_open: false,
            quit_confirm: false,
            quitting: false,
            stats_stop: stop,
            connect_hint: None,
        };
        app.push_log("openfortivpn GUI window session started.".into());
        app
    }

    pub fn push_log(&self, line: String) {
        self.shared.push_log(line);
    }

    /// Log the event and fire a desktop notification off-thread.
    fn push_notify(&self, summary: &str, body: String) {
        self.push_log(format!("[notify] {summary} — {body}"));
        let summary = summary.to_owned();
        std::thread::spawn(move || {
            if let Err(e) = util::notify(&summary, &body) {
                eprintln!("notification failed: {e}");
            }
        });
    }

    fn drain_events(&mut self) {
        let incoming: Vec<UiEvent> = match self.shared.events.lock() {
            Ok(mut q) => q.drain(..).collect(),
            Err(_) => Vec::new(),
        };
        for ev in incoming {
            self.apply_event(ev);
        }
    }

    fn apply_event(&mut self, ev: UiEvent) {
        match ev {
            UiEvent::Log(l) => self.push_log(l),
            UiEvent::Stats { stats, iface } => {
                self.state.stats = Some(stats);
                self.state.stats_iface = Some(iface);
            }
            UiEvent::Phase(ph) => {
                if let ConnPhase::Connected { iface } = &ph {
                    if matches!(self.state.phase, ConnPhase::Connecting) {
                        let iface = iface.clone();
                        self.push_log(format!("Connected — interface {iface}."));
                        self.state.phase = ph;
                        self.push_notify("VPN connected", format!("Interface {iface}"));
                    }
                }
            }
            UiEvent::IfaceGone => {
                if self.state.phase.connected() {
                    self.push_log("VPN interface disappeared.".into());
                    self.state.phase = ConnPhase::Idle;
                    self.state.stats = None;
                    self.state.stats_iface = None;
                    self.push_notify("VPN disconnected", "The VPN interface disappeared.".into());
                }
            }
            UiEvent::CertUnknown(h) => self.state.unknown_cert = Some(h),
            UiEvent::SudoAuthFailed => {
                self.sudo_session = None;
                self.state.stats = None;
                self.state.phase = ConnPhase::Failed {
                    reason: "sudo rejected the password (cached sudo password cleared).".into(),
                };
                self.push_notify(
                    "Connection failed",
                    "sudo rejected the password (cached sudo password cleared).".into(),
                );
            }
            UiEvent::ConnectFailed(m) => {
                if matches!(self.state.phase, ConnPhase::Connecting) {
                    self.stats_stop.store(true, Ordering::SeqCst);
                    self.shared.vpn.disconnect();
                    self.state.phase = ConnPhase::Failed { reason: m.clone() };
                    self.push_notify("Connection failed", m);
                }
            }
            UiEvent::VpnExited(code) => {
                self.stats_stop.store(true, Ordering::SeqCst);
                match &self.state.phase {
                    ConnPhase::Connecting => {
                        let reason = format!("openfortivpn exited with code {code}.");
                        self.state.phase = ConnPhase::Failed {
                            reason: reason.clone(),
                        };
                        self.state.stats = None;
                        self.push_notify("Connection failed", reason);
                    }
                    ConnPhase::Connected { .. } | ConnPhase::External { .. } => {
                        self.push_log("VPN process exited.".into());
                        self.state.phase = ConnPhase::Idle;
                        self.state.stats = None;
                        self.state.stats_iface = None;
                        self.push_notify("VPN disconnected", "The VPN process exited.".into());
                    }
                    _ => {}
                }
            }
        }
    }

    fn drain_tray(&mut self) {
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            match ev.id().0.as_str() {
                "open" => self.show_window(),
                "toggle" => self.tray_toggle(),
                "exit" => self.request_quit(),
                _ => {}
            }
        }
    }

    fn drain_instance(&mut self) {
        for ev in util::single_instance::drain(&self.shared.inst_events) {
            if matches!(ev, InstEvent::Show) {
                self.show_window();
            }
        }
    }

    fn sync_tray(&mut self) {
        let Some(tray) = self.shared.tray.as_ref() else {
            return;
        };
        let (variant, tip) = match &self.state.phase {
            ConnPhase::Connected { iface } | ConnPhase::External { iface, .. } => (
                TrayVariant::Connected,
                format!("openfortivpn: connected ({iface})"),
            ),
            ConnPhase::Connecting => (
                TrayVariant::Connecting,
                "openfortivpn: connecting…".to_owned(),
            ),
            ConnPhase::Failed { .. } => (TrayVariant::Error, "openfortivpn: failed".to_owned()),
            ConnPhase::Idle => (TrayVariant::Idle, "openfortivpn: idle".to_owned()),
        };
        tray.set_state(variant, &tip);
        tray.set_toggle(self.state.phase.busy());
        let status = if self.state.phase.connected() {
            match &self.state.stats {
                Some(s) => format!(
                    "↓ {} MB ↑ {} MB · {} KB/s",
                    fmt_mb(s.rx_bytes),
                    fmt_mb(s.tx_bytes),
                    fmt_kb(s.rx_kbps + s.tx_kbps)
                ),
                None => "Connected — collecting stats…".to_owned(),
            }
        } else {
            "Not connected".to_owned()
        };
        tray.set_status(&status);
    }

    pub fn show_window(&self) {
        if let Some(ctx) = &self.ctx {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
    }

    fn tray_toggle(&mut self) {
        if self.state.phase.busy() {
            self.on_disconnect();
            return;
        }
        if self.vpn_pass.is_empty() {
            self.connect_hint = Some("Type your VPN password to connect.".into());
            self.show_window();
            return;
        }
        self.on_connect();
    }

    pub fn on_connect(&mut self) {
        if !self.state.installed || self.state.phase.busy() {
            return;
        }
        let Some(profile) = self.book.selected_profile().cloned() else {
            return;
        };
        if self.vpn_pass.is_empty() {
            self.connect_hint = Some("Type your VPN password to connect.".into());
            return;
        }
        self.connect_hint = None;
        if self.state.passwordless {
            self.do_connect(profile, None);
        } else if let Some(sp) = self.sudo_session.clone() {
            self.do_connect(profile, Some(sp));
        } else {
            self.pending_connect = Some(PendingConnect {
                profile,
                vpn_pass: Zeroizing::new(self.vpn_pass.clone()),
            });
        }
    }

    pub fn do_connect(&mut self, profile: Profile, sudo_pass: Option<Zeroizing<String>>) {
        let Some(mode) = self.current_mode() else {
            return;
        };
        self.connect_hint = None;
        self.stats_stop.store(true, Ordering::SeqCst);
        self.state.unknown_cert = None;
        self.state.stats = None;
        self.state.stats_iface = None;
        self.state.phase = ConnPhase::Connecting;
        let stop = Arc::new(AtomicBool::new(false));
        self.stats_stop = Arc::clone(&stop);
        self.shared.vpn.connect(
            profile.clone(),
            Zeroizing::new(self.vpn_pass.clone()),
            sudo_pass,
            mode,
        );
        stats::spawn(
            profile.interface.clone(),
            None,
            Arc::clone(&self.shared.events),
            stop,
        );
        self.push_log(format!(
            "Connecting to {} as {}…",
            profile.server, profile.username
        ));
    }

    pub fn on_disconnect(&mut self) {
        self.stats_stop.store(true, Ordering::SeqCst);
        match self.state.phase.clone() {
            ConnPhase::External { pid, .. } => {
                self.shared.vpn.kill_external(pid);
                self.push_log(format!("Terminating external openfortivpn (pid {pid})…"));
            }
            ConnPhase::Connecting | ConnPhase::Connected { .. } => {
                self.shared.vpn.disconnect();
                self.push_log("Disconnect requested…".into());
            }
            _ => return,
        }
        self.state.phase = ConnPhase::Idle;
        self.state.stats = None;
        self.state.stats_iface = None;
        self.push_notify("VPN disconnected", "Disconnect requested.".into());
    }

    pub fn request_quit(&mut self) {
        if self.state.phase.busy() {
            self.quit_confirm = true;
        } else {
            self.do_quit();
        }
    }

    pub fn do_quit(&mut self) {
        self.stats_stop.store(true, Ordering::SeqCst);
        if self.state.phase.busy() {
            self.on_disconnect();
        }
        self.shared.vpn.quit();
        self.quitting = true;
        if let Some(ctx) = self.ctx.clone() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    pub fn confirm_sudo(&mut self) {
        if self.sudo_buf.is_empty() {
            return;
        }
        self.sudo_session = Some(Zeroizing::new(self.sudo_buf.clone()));
        self.sudo_buf.clear();
        if let Some(p) = self.pending_connect.take() {
            self.do_connect(p.profile, self.sudo_session.clone());
        }
    }

    pub fn cancel_sudo(&mut self) {
        self.pending_connect = None;
        self.sudo_buf.clear();
    }

    pub fn current_mode(&self) -> Option<SudoProbe> {
        let b = self.state.binary_path.clone()?;
        Some(if self.state.passwordless {
            SudoProbe::Passwordless(b)
        } else {
            SudoProbe::NeedsPassword(b)
        })
    }

    pub fn reprobe(&mut self) {
        let probe = crate::controller::sudo::probe();
        match &probe {
            SudoProbe::MissingBinary => {
                self.state.installed = false;
                self.state.passwordless = false;
                self.state.binary_path = None;
            }
            SudoProbe::NeedsPassword(b) => {
                self.state.installed = true;
                self.state.passwordless = false;
                self.state.binary_path = Some(b.clone());
            }
            SudoProbe::Passwordless(b) => {
                self.state.installed = true;
                self.state.passwordless = true;
                self.state.binary_path = Some(b.clone());
            }
        }
        self.push_log(format!(
            "Re-checked: installed={}, passwordless={}",
            self.state.installed, self.state.passwordless
        ));
    }

    pub fn apply_editor(&mut self, prof: Profile) {
        let id = prof.id.clone();
        self.book.upsert(prof);
        self.book.selected = Some(id);
        self.book.save(&self.shared.paths.book_path).ok();
    }

    pub fn delete_editor(&mut self) {
        if let Some(id) = self.book.selected.clone() {
            self.book.delete(&id);
            self.book.save(&self.shared.paths.book_path).ok();
        }
    }
}

impl eframe::App for OFGApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ctx = Some(ctx.clone());
        if !self.ticker_started {
            self.ticker_started = true;
            let c = ctx.clone();
            std::thread::spawn(move || loop {
                c.request_repaint_after(Duration::from_millis(250));
                std::thread::sleep(Duration::from_millis(250));
            });
        }

        // Debug/smoke hook (AGENTS.md §14): OFG_SMOKE_CLOSE_MS=<ms> closes the
        // window session automatically so close-to-tray/reopen can be tested
        // programmatically.
        if !self.smoke_done {
            match std::env::var("OFG_SMOKE_CLOSE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            {
                Some(ms) => {
                    if self.session_started.elapsed() >= Duration::from_millis(ms) {
                        self.smoke_done = true;
                        self.push_log("[smoke] simulating window close".into());
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
                None => self.smoke_done = true,
            }
        }

        // Close (X): hide from screen but keep the app + VPN alive.
        // X11: real hide (winit unmap/map works). Wayland: winit cannot hide,
        // unmap, restore or focus a window (set_visible is a no-op,
        // focus_window is empty, unminimize is ignored) — so we minimize to
        // the taskbar instead; restoring is done from the taskbar.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting {
                dbg_log("close_requested → QUIT");
                // No CancelClose: let eframe tear down and return.
            } else {
                dbg_log(if self.on_wayland {
                    "close_requested → minimize (Wayland)"
                } else {
                    "close_requested → hide to tray (X11)"
                });
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                if self.on_wayland {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                } else {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                }
                self.push_log("Window hidden — openfortivpn-gui keeps running in the tray.".into());
            }
        }

        // Heartbeat (stderr, every 2 s) — proves the event loop keeps ticking
        // (tray menu liveness) even while the window is hidden/minimized.
        if self.session_started.elapsed() - self.last_heartbeat >= Duration::from_secs(2) {
            self.last_heartbeat = self.session_started.elapsed();
            dbg_log("heartbeat");
        }

        self.drain_events();
        self.drain_tray();
        self.drain_instance();
        self.sync_tray();
        if self.shared.tray.is_some() {
            // Pump GTK so libayatana-appindicator delivers menu events/updates.
            gtk::main_iteration_do(false);
        }
        main_window::draw(self, ctx);
    }
}
