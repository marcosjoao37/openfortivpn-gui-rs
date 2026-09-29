use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use uuid::Uuid;

/// One VPN connection profile. Passwords are NEVER stored here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// "host" or "host:port" (port defaults to 443).
    pub server: String,
    pub username: String,
    #[serde(default)]
    pub trust_cert: bool,
    #[serde(default)]
    pub trusted_cert: String,
    /// Optional interface override (e.g. "ppp0"); None = autodetect.
    #[serde(default)]
    pub interface: Option<String>,
}

impl Profile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            server: String::new(),
            username: String::new(),
            trust_cert: false,
            trusted_cert: String::new(),
            interface: None,
        }
    }

    /// Split "host[:port]" → (host, port).
    pub fn host_port(&self) -> (String, u16) {
        let server = self.server.trim();
        match server.rsplit_once(':') {
            Some((h, p))
                if !h.is_empty() && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) =>
            {
                (h.to_owned(), p.parse().unwrap_or(443))
            }
            _ => (server.to_owned(), 443),
        }
    }

    pub fn valid(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Profile name is required.".into());
        }
        if self.server.trim().is_empty() {
            return Err("Server is required (host or host:port).".into());
        }
        if self.username.trim().is_empty() {
            return Err("Username is required.".into());
        }
        if self.trust_cert {
            let c = self.trusted_cert.trim();
            if c.len() != 64 || !c.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err(
                    "Trusted certificate must be a SHA-256 digest (64 hex characters).".into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileBook {
    pub version: u32,
    #[serde(default)]
    pub selected: Option<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl ProfileBook {
    /// Load from disk; Ok(None) when the file does not exist yet.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        match fs::read_to_string(path) {
            Ok(txt) => serde_json::from_str(&txt)
                .map(Some)
                .map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Atomic save: temp file (0600) + rename. Directory gets 0700.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let dir = path.parent().ok_or("profile path has no parent")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|e| e.to_string())?;
            f.write_all(
                serde_json::to_string_pretty(self)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn upsert(&mut self, profile: Profile) {
        match self.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(slot) => *slot = profile,
            None => self.profiles.push(profile),
        }
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        let removed = self.profiles.len() != before;
        if self.selected.as_deref() == Some(id) {
            self.selected = self.profiles.first().map(|p| p.id.clone());
        }
        removed
    }

    pub fn selected_profile(&self) -> Option<&Profile> {
        let sel = self.selected.as_deref()?;
        self.get(sel)
    }

    /// First-run seed from the user's reference command.
    pub fn seed_default() -> Self {
        let mut b = Self {
            version: 1,
            selected: None,
            profiles: vec![],
        };
        let mut p = Profile::new("CETIQT");
        p.server = "vpn2.cetiqt.senai.br:10443".into();
        p.username = "joao.araujo".into();
        p.trust_cert = true;
        p.trusted_cert = "10adad89836d4dbb03570776d469473ddb5dd25ceb6b60826f174275d8c1cfd3".into();
        b.profiles.push(p);
        b.selected = b.profiles.first().map(|p| p.id.clone());
        b
    }
}
