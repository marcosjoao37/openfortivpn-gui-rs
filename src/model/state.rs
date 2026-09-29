use std::collections::VecDeque;
use std::path::PathBuf;

/// Throughput snapshot; byte totals are per-session (since iface detection).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_kbps: f64,
    pub tx_kbps: f64,
}

impl Stats {
    pub fn down_mb(&self) -> f64 {
        self.rx_bytes as f64 / 1_048_576.0
    }
    pub fn up_mb(&self) -> f64 {
        self.tx_bytes as f64 / 1_048_576.0
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ConnPhase {
    #[default]
    Idle,
    Connecting,
    Connected {
        iface: String,
    },
    External {
        pid: u32,
        iface: String,
    },
    Failed {
        reason: String,
    },
}

impl ConnPhase {
    /// A session exists or is being established.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            ConnPhase::Connecting | ConnPhase::Connected { .. } | ConnPhase::External { .. }
        )
    }
    pub fn connected(&self) -> bool {
        matches!(
            self,
            ConnPhase::Connected { .. } | ConnPhase::External { .. }
        )
    }
}

/// Immutable-ish snapshot the view reads; controllers push events to mutate it.
#[derive(Debug, Default)]
pub struct UiState {
    pub phase: ConnPhase,
    pub stats: Option<Stats>,
    pub stats_iface: Option<String>,
    pub log: VecDeque<String>,
    pub installed: bool,
    pub passwordless: bool,
    pub binary_path: Option<PathBuf>,
    pub unknown_cert: Option<String>,
}

impl UiState {
    pub fn push_log(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > 500 {
            self.log.pop_front();
        }
    }
}

pub fn fmt_mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

pub fn fmt_kb(bps: f64) -> String {
    format!("{:.0}", bps / 1024.0)
}
