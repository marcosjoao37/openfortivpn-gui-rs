use openfortivpn_gui::controller::sudo;
use openfortivpn_gui::controller::tray;
use openfortivpn_gui::controller::vpn::{UiEvent, VpnController};
use openfortivpn_gui::model::distro;
use openfortivpn_gui::model::profile::ProfileBook;
use openfortivpn_gui::util;
use openfortivpn_gui::util::single_instance;
use openfortivpn_gui::view::{OFGApp, Parts};
use std::sync::mpsc::channel;

fn main() -> eframe::Result<()> {
    let paths = util::paths();

    // Single instance: second launch asks the first one to show its window.
    let inst_rx = match single_instance::acquire(&paths.sock) {
        Some(primary) => Some(primary.into_events()),
        None => {
            eprintln!("openfortivpn-gui is already running — asked it to show its window.");
            return Ok(());
        }
    };

    let gtk_ok = gtk::init().is_ok();

    std::fs::create_dir_all(&paths.config_dir).ok();
    let mut book = match ProfileBook::load(&paths.book_path) {
        Ok(Some(b)) => b,
        Ok(None) => ProfileBook::seed_default(),
        Err(e) => {
            eprintln!("profiles.json unreadable ({e}); starting fresh");
            ProfileBook::seed_default()
        }
    };
    if book.selected.is_none() {
        book.selected = book.profiles.first().map(|p| p.id.clone());
    }
    book.save(&paths.book_path).ok();

    let probe = sudo::probe();

    let (evt_tx, evt_rx) = channel::<UiEvent>();
    let vpn = VpnController::new(evt_tx.clone());
    let tray = tray::init(gtk_ok).ok();

    let os = distro::detect();
    let distro_pretty = os
        .as_ref()
        .map(|d| d.pretty.clone())
        .unwrap_or_else(|| "unknown".into());
    let distro_cmd = os
        .as_ref()
        .map(|d| d.install_cmd.to_string())
        .unwrap_or_else(|| distro::install_command("", "").to_string());

    let parts = Parts {
        book,
        evt_rx,
        evt_tx,
        vpn,
        tray,
        inst_rx,
        paths,
        probe,
        distro_pretty,
        distro_cmd,
    };

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

    eframe::run_native(
        "openfortivpn GUI",
        options,
        Box::new(move |cc| Ok(Box::new(OFGApp::new(cc, parts)))),
    )
}
