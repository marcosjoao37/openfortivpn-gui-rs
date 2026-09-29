use openfortivpn_gui::controller::sudo;
use openfortivpn_gui::controller::tray;
use openfortivpn_gui::controller::vpn::VpnController;
use openfortivpn_gui::model::distro;
use openfortivpn_gui::model::profile::ProfileBook;
use openfortivpn_gui::util;
use openfortivpn_gui::util::single_instance;
use openfortivpn_gui::view::{OFGApp, Parts, SharedRuntime};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn main() -> eframe::Result<()> {
    let paths = util::paths();

    // Single instance: second launch asks the first one to show its window.
    let Some(primary) = single_instance::acquire(&paths.sock) else {
        eprintln!("openfortivpn-gui is already running — asked it to show its window.");
        return Ok(());
    };

    // Window backend: prefer X11 (XWayland) even under a Wayland session.
    // winit's Wayland backend cannot hide, restore or focus windows (all
    // no-ops), which breaks close-to-tray; the X11 backend supports real
    // unmap/map. Without any X11 display we stay on Wayland and degrade to
    // minimize-on-close (see view::OFGApp close handling).
    let has_x11 =
        std::env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false);
    if has_x11 {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
    openfortivpn_gui::view::dbg_log(if has_x11 {
        "window backend: X11/XWayland — close hides to tray"
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
        log: Arc::new(Mutex::new(VecDeque::new())),
        distro_pretty,
        distro_cmd,
    });
    shared.push_log("openfortivpn GUI started.".into());

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

    let result = eframe::run_native(
        "openfortivpn GUI",
        options,
        Box::new(move |cc| Ok(Box::new(OFGApp::new(cc, parts)))),
    );

    // Cleanup: tear down any VPN session this process started or adopted.
    if let Some(pid) = util::find_pids("openfortivpn").first() {
        shared.vpn.kill_external(*pid);
    }
    shared.vpn.disconnect();
    shared.vpn.quit();
    result
}
