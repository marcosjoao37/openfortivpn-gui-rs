use crate::controller::vpn::{push_event, EventQueue, UiEvent};
use crate::model::state::{ConnPhase, Stats};
use crate::util;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Spawn the sampler thread.
/// - `existing: Some(iface)` → adopt an already-live interface (external session);
///   no Phase event is emitted (the view owns that state).
/// - `existing: None` → wait for a NEW ppp*/tun* interface, then emit Phase(Connected).
pub fn spawn(
    interface_override: Option<String>,
    existing: Option<String>,
    events: EventQueue,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || run(interface_override, existing, events, stop));
}

fn run(
    override_iface: Option<String>,
    existing: Option<String>,
    events: EventQueue,
    stop: Arc<AtomicBool>,
) {
    let iface = match existing {
        Some(i) => i,
        None => match wait_for_iface(override_iface, &events, &stop) {
            Some(i) => {
                push_event(
                    &events,
                    UiEvent::Phase(ConnPhase::Connected { iface: i.clone() }),
                );
                i
            }
            None => return, // timeout event already sent
        },
    };

    let Some(base) = util::iface_bytes(&iface) else {
        push_event(&events, UiEvent::IfaceGone);
        return;
    };
    let mut prev = base;

    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        // 1 s window: ten 100 ms slices so stop requests are honored promptly.
        for _ in 0..10 {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let Some(now) = util::iface_bytes(&iface) else {
            push_event(&events, UiEvent::IfaceGone);
            return;
        };
        let rx_total = now.0.saturating_sub(base.0);
        let tx_total = now.1.saturating_sub(base.1);
        let rx_rate = now.0.saturating_sub(prev.0) as f64; // bytes in the 1 s window
        let tx_rate = now.1.saturating_sub(prev.1) as f64;
        prev = now;
        push_event(
            &events,
            UiEvent::Stats {
                stats: Stats {
                    rx_bytes: rx_total,
                    tx_bytes: tx_total,
                    rx_kbps: rx_rate / 1024.0,
                    tx_kbps: tx_rate / 1024.0,
                },
                iface: iface.clone(),
            },
        );
    }
}

/// Poll up to 120 s for a ppp*/tun* interface that did not exist at spawn time.
fn wait_for_iface(
    override_iface: Option<String>,
    events: &EventQueue,
    stop: &Arc<AtomicBool>,
) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    if let Some(name) = &override_iface {
        // Override: wait for it to simply exist.
        while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
            if util::iface_bytes(name).is_some() {
                return Some(name.clone());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        push_event(
            events,
            UiEvent::ConnectFailed(format!("interface '{name}' did not appear within 120 s.")),
        );
        return None;
    }
    let baseline: std::collections::HashSet<String> = util::net_ifaces().into_iter().collect();
    while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
        if let Some(name) = util::vpn_ifaces()
            .into_iter()
            .find(|i| !baseline.contains(i))
        {
            return Some(name);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if !stop.load(Ordering::SeqCst) {
        push_event(
            events,
            UiEvent::ConnectFailed("no VPN interface (ppp*/tun*) appeared within 120 s.".into()),
        );
    }
    None
}
