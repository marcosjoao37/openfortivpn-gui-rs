use std::sync::{Arc, Mutex};
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

#[derive(Default)]
struct TrayCache {
    variant: Option<TrayVariant>,
    toggle: Option<bool>,
    status: Option<String>,
}

/// Shared tray handle: lives across window sessions, so all mutators take
/// `&self` and cache state internally to avoid redundant DBus churn.
pub struct TrayApp {
    tray: TrayIcon,
    item_toggle: MenuItem,
    item_status: MenuItem,
    icons: [TrayIconData; 4],
    cache: Mutex<TrayCache>,
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
        tray,
        item_toggle: toggle,
        item_status: status,
        icons,
        cache: Mutex::new(TrayCache::default()),
    })
}

impl TrayApp {
    pub fn set_state(&self, variant: TrayVariant, tooltip: &str) {
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        if cache.variant == Some(variant) {
            return;
        }
        let idx = match variant {
            TrayVariant::Idle => 0,
            TrayVariant::Connecting => 1,
            TrayVariant::Connected => 2,
            TrayVariant::Error => 3,
        };
        if let Ok(icon) = make_icon(&self.icons[idx]) {
            let _ = self.tray.set_icon(Some(icon));
            let _ = self.tray.set_tooltip(Some(tooltip.to_owned()));
        }
        cache.variant = Some(variant);
    }

    pub fn set_toggle(&self, connected: bool) {
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        if cache.toggle != Some(connected) {
            self.item_toggle
                .set_text(if connected { "Disconnect" } else { "Connect" });
            cache.toggle = Some(connected);
        }
    }

    pub fn set_status(&self, text: &str) {
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        if cache.status.as_deref() != Some(text) {
            self.item_status.set_text(text);
            cache.status = Some(text.to_owned());
        }
    }
}
