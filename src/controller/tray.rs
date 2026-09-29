use std::sync::Arc;
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayVariant {
    Idle,
    Connecting,
    Connected,
    Error,
}

#[derive(Clone)]
pub struct TrayIconData {
    pub rgba: Arc<Vec<u8>>,
    pub w: u32,
    pub h: u32,
}

pub struct TrayApp {
    _tray: TrayIcon,
    item_toggle: MenuItem,
    item_status: MenuItem,
    icons: [TrayIconData; 4],
    last_variant: Option<TrayVariant>,
    last_toggle: Option<bool>,
    last_status: String,
}

fn decode(png: &[u8], what: &str) -> Result<TrayIconData, String> {
    let (rgba, w, h) = crate::util::png_rgba(png).map_err(|e| format!("{what}: {e}"))?;
    Ok(TrayIconData { rgba, w, h })
}

fn make_icon(d: &TrayIconData) -> Result<Icon, String> {
    Icon::from_rgba(d.rgba.to_vec(), d.w, d.h).map_err(|e| e.to_string())
}

pub fn init(gtk_ok: bool) -> Result<TrayApp, String> {
    if !gtk_ok {
        return Err("GTK unavailable; tray disabled".into());
    }
    let icons = [
        decode(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/tray-idle-32.png"
            )),
            "tray-idle",
        )?,
        decode(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/tray-connecting-32.png"
            )),
            "tray-connecting",
        )?,
        decode(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/tray-connected-32.png"
            )),
            "tray-connected",
        )?,
        decode(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/tray-error-32.png"
            )),
            "tray-error",
        )?,
    ];

    let open = MenuItem::with_id("open", "Open", true, None);
    let toggle = MenuItem::with_id("toggle", "Connect", true, None);
    let status = MenuItem::with_id("status", "Not connected", false, None);
    let quit = MenuItem::with_id("exit", "Exit", true, None);

    let menu = Menu::new();
    menu.append(&open).map_err(|e| e.to_string())?;
    menu.append(&toggle).map_err(|e| e.to_string())?;
    menu.append(&status).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    let tray = TrayIconBuilder::new()
        .with_id("ofg-tray")
        .with_menu(Box::new(menu))
        .with_tooltip("openfortivpn GUI")
        .with_icon(make_icon(&icons[0])?)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(TrayApp {
        _tray: tray,
        item_toggle: toggle,
        item_status: status,
        icons,
        last_variant: None,
        last_toggle: None,
        last_status: String::new(),
    })
}

impl TrayApp {
    pub fn set_state(&mut self, variant: TrayVariant, tooltip: &str) {
        if self.last_variant == Some(variant) {
            return;
        }
        let idx = match variant {
            TrayVariant::Idle => 0,
            TrayVariant::Connecting => 1,
            TrayVariant::Connected => 2,
            TrayVariant::Error => 3,
        };
        if let Ok(icon) = make_icon(&self.icons[idx]) {
            let _ = self._tray.set_icon(Some(icon));
            let _ = self._tray.set_tooltip(Some(tooltip.to_owned()));
        }
        self.last_variant = Some(variant);
    }

    pub fn set_toggle(&mut self, connected: bool) {
        if self.last_toggle != Some(connected) {
            self.item_toggle
                .set_text(if connected { "Disconnect" } else { "Connect" });
            self.last_toggle = Some(connected);
        }
    }

    pub fn set_status(&mut self, text: &str) {
        if self.last_status != text {
            self.item_status.set_text(text);
            self.last_status = text.to_owned();
        }
    }
}
