use openfortivpn_gui::controller::sudo;
use openfortivpn_gui::controller::tray::{self, TrayVariant};
use openfortivpn_gui::controller::vpn::{UiEvent, VpnController};
use openfortivpn_gui::model::distro;
use openfortivpn_gui::model::profile::ProfileBook;
use openfortivpn_gui::util;
use openfortivpn_gui::util::single_instance::{self, InstEvent};
use openfortivpn_gui::view::{dbg_log, OFGApp, Parts, SharedRuntime, SESSION_QUIT};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::menu::MenuEvent;

fn main() -> eframe::Result<()> {
    let paths = util::paths();

    // Single instance: second launch asks the first one to show its window.
    let Some(primary) = single_instance::acquire(&paths.sock) else {
        eprintln!("openfortivpn-gui is already running — asked it to show its window.");
        return Ok(());
    };

    // Window backend: prefer X11 (XWayland) even under a Wayland session.
    // winit's Wayland backend cannot hide, restore or focus windows (all
    // no-ops), which breaks close-to-tray. Under X11 the eframe loop exit
    // closes the X connection, destroying the window for real — so the
    // session loop below can destroy/recreate windows freely. Without any
    // X11 display we stay on Wayland and degrade to minimize-on-close.
    let has_x11 = std::env::var("DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if has_x11 {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
    dbg_log(if has_x11 {
        "window backend: X11/XWayland — close destroys the window session"
    } else {
        "window backend: Wayland — close minimizes (no X11 display)"
    });

    let gtk_ok = gtk::init().is_ok();
    let tray = tray::init(gtk_ok).ok();

    std::fs::create_dir_all(&paths.config_dir).ok();

    let events = Arc::new(Mutex::new(VecDeque::new()));
    let vpn = VpnController::new(Arc::clone(&events));

    let os = distro::detect();
    let distro_pretty = os
        .as_ref()
        .map(|d| d.pretty.clone())
        .unwrap_or_else(|| "unknown".into());
    let distro_cmd = os
        .as_ref()
        .map(|d| d.install_cmd.to_string())
        .unwrap_or_else(|| distro::install_command("", "").to_string());

    let shared = Rc::new(SharedRuntime {
        paths: paths.clone(),
        vpn,
        tray,
        events,
        inst_events: primary.events,
        session_end: Arc::new(std::sync::atomic::AtomicU8::new(0)),
        log: Arc::new(Mutex::new(VecDeque::new())),
        distro_pretty,
        distro_cmd,
    });
    shared.push_log("openfortivpn GUI started.".into());

    // Window-session loop. Close (X) ends the eframe window but the process
    // (tray, VPN controller, log) survives in the headless pump loop below;
    // Open/Connect from the tray or a second launch creates a new window.
    loop {
        shared.session_end.store(0, Ordering::SeqCst);

        let book = match ProfileBook::load(&paths.book_path) {
            Ok(Some(b)) => b,
            Ok(None) | Err(_) => ProfileBook::seed_default(),
        };
        let mut book = book;
        if book.selected.is_none() {
            book.selected = book.profiles.first().map(|p| p.id.clone());
        }

        let icon = util::icon_data(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icons/openfortivpn-gui-128.png"
        )))
        .expect("app icon missing — run `make icons`");

        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([640.0, 680.0])
                .with_min_inner_size([560.0, 540.0])
                .with_icon(icon),
            ..Default::default()
        };

        let parts = Parts {
            shared: Rc::clone(&shared),
            book,
            probe: sudo::probe(),
        };

        eframe::run_native(
            "openfortivpn GUI",
            options,
            Box::new(move |cc| Ok(Box::new(OFGApp::new(cc, parts)))),
        )?;

        match shared.session_end.load(Ordering::SeqCst) {
            SESSION_QUIT => break,
            _ => match pump_until_reopen(&shared) {
                Pump::Reopen => continue,
                Pump::Quit => break,
            },
        }
    }

    // Final cleanup: tear down any VPN session this process started or adopted.
    if let Some(pid) = util::find_pids("openfortivpn").first() {
        shared.vpn.kill_external(*pid);
    }
    shared.vpn.disconnect();
    shared.vpn.quit();
    Ok(())
}

enum Pump {
    Reopen,
    Quit,
}

/// Headless tray loop between window sessions: pumps GTK, handles the tray
/// menu (Open/Connect/Disconnect/Exit) and the single-instance queue, keeps
/// the log fed, and reflects the connection state in the tray icon.
fn pump_until_reopen(shared: &SharedRuntime) -> Pump {
    dbg_log("pump: headless tray loop entered");
    loop {
        if shared.tray.is_some() {
            gtk::main_iteration_do(false);
        }
        std::thread::sleep(Duration::from_millis(50));

        for ev in single_instance::drain(&shared.inst_events) {
            if matches!(ev, InstEvent::Show) {
                dbg_log("pump: reopen via second instance");
                return Pump::Reopen;
            }
        }

        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            dbg_log(&format!("pump: tray menu event: {}", ev.id().0));
            match ev.id().0.as_str() {
                "open" => return Pump::Reopen,
                "exit" => return Pump::Quit,
                "toggle" => {
                    let pids = util::find_pids("openfortivpn");
                    if let Some(pid) = pids.first() {
                        dbg_log(&format!("pump: tray Disconnect (pid {pid})"));
                        shared.vpn.kill_external(*pid);
                        shared.vpn.disconnect();
                        shared.push_log("Disconnect requested from tray.".into());
                        let _ = util::notify("VPN disconnected", "Disconnect requested from tray.");
                    } else {
                        dbg_log("pump: tray Connect → reopening window for password");
                        // Connecting needs a password → open the window.
                        return Pump::Reopen;
                    }
                }
                _ => {}
            }
        }

        // Keep the persistent log fed; surface failures happening headless.
        let drained: Vec<UiEvent> = shared
            .events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        for ev in drained {
            match ev {
                UiEvent::Log(l) => shared.push_log(l),
                UiEvent::VpnExited(code) => {
                    shared.push_log(format!("VPN process exited (code {code})."));
                    let _ = util::notify("VPN disconnected", "The VPN process exited.");
                }
                UiEvent::IfaceGone => {
                    shared.push_log("VPN interface disappeared.".into());
                    let _ = util::notify("VPN disconnected", "The VPN interface disappeared.");
                }
                UiEvent::SudoAuthFailed => {
                    shared.push_log("sudo rejected the password.".into());
                    let _ = util::notify("Connection failed", "sudo rejected the password.");
                }
                _ => {}
            }
        }

        // Light tray sync: icon color + connect/disconnect label.
        if let Some(t) = shared.tray.as_ref() {
            let connected = !util::find_pids("openfortivpn").is_empty();
            if connected {
                t.set_state(TrayVariant::Connected, "openfortivpn: running in tray");
            } else {
                t.set_state(TrayVariant::Idle, "openfortivpn: idle (running in tray)");
            }
            t.set_toggle(connected);
            t.set_status(if connected {
                "Running in tray"
            } else {
                "Not connected"
            });
        }
    }
}
