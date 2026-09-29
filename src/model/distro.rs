/// Detected distribution + package-manager install command.
pub struct Distro {
    pub id: String,
    pub pretty: String,
    pub install_cmd: &'static str,
}

/// Map os-release ID/ID_LIKE → install command.
pub fn install_command(id: &str, id_like: &str) -> &'static str {
    let hay = format!("{id} {id_like}").to_lowercase();
    if hay.contains("arch") {
        "sudo pacman -S openfortivpn"
    } else if hay.contains("debian") || hay.contains("ubuntu") || hay.contains("mint") {
        "sudo apt install openfortivpn"
    } else if hay.contains("fedora") || hay.contains("rhel") {
        "sudo dnf install openfortivpn"
    } else if hay.contains("suse") {
        "sudo zypper install openfortivpn"
    } else if hay.contains("alpine") {
        "sudo apk add openfortivpn"
    } else if hay.contains("gentoo") {
        "sudo emerge --ask net-vpn/openfortivpn"
    } else {
        "Build from source: https://github.com/adrienverge/openfortivpn"
    }
}

/// Parse /etc/os-release (ID, ID_LIKE, PRETTY_NAME).
pub fn detect() -> Option<Distro> {
    let txt = std::fs::read_to_string("/etc/os-release").ok()?;
    let mut id = String::new();
    let mut id_like = String::new();
    let mut pretty = String::new();
    for line in txt.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            id = v.trim_matches('"').to_owned();
        } else if let Some(v) = line.strip_prefix("ID_LIKE=") {
            id_like = v.trim_matches('"').to_owned();
        } else if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
            pretty = v.trim_matches('"').to_owned();
        }
    }
    if id.is_empty() {
        return None;
    }
    Some(Distro {
        install_cmd: install_command(&id, &id_like),
        id,
        pretty,
    })
}

pub const SUDOERS_EDIT_CMD: &str = "sudo EDITOR=nano visudo";

/// sudoers line granting NOPASSWD for the resolved binary path.
pub fn sudoers_rule(user: &str, bin: &str) -> String {
    format!("{user} ALL=(root) NOPASSWD: {bin}")
}
