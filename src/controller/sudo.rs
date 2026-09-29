use crate::util;
use std::path::PathBuf;
use std::process::Command;

/// Result of checking whether `sudo openfortivpn` can run without a password.
#[derive(Debug, Clone)]
pub enum SudoProbe {
    MissingBinary,
    Passwordless(PathBuf),
    NeedsPassword(PathBuf),
}

impl SudoProbe {
    pub fn binary(&self) -> Option<&PathBuf> {
        match self {
            SudoProbe::Passwordless(b) | SudoProbe::NeedsPassword(b) => Some(b),
            SudoProbe::MissingBinary => None,
        }
    }
    pub fn passwordless(&self) -> bool {
        matches!(self, SudoProbe::Passwordless(_))
    }
}

/// Locale-independent probe: `sudo -n <binary> --version` succeeds only when
/// the openfortivpn rule is NOPASSWD (or the user is passwordless for it).
pub fn probe() -> SudoProbe {
    let Some(bin) = util::which("openfortivpn") else {
        return SudoProbe::MissingBinary;
    };
    let ok = Command::new("sudo")
        .arg("-n")
        .arg(&bin)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        SudoProbe::Passwordless(bin)
    } else {
        SudoProbe::NeedsPassword(bin)
    }
}
