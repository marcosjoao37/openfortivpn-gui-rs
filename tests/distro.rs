use openfortivpn_gui::model::distro::{install_command, sudoers_rule, SUDOERS_EDIT_CMD};

#[test]
fn arch_like_uses_pacman() {
    assert_eq!(
        install_command("cachyos", "arch"),
        "sudo pacman -S openfortivpn"
    );
    assert_eq!(install_command("arch", ""), "sudo pacman -S openfortivpn");
    assert_eq!(
        install_command("manjaro", "arch"),
        "sudo pacman -S openfortivpn"
    );
}

#[test]
fn debian_like_uses_apt() {
    assert_eq!(
        install_command("ubuntu", "debian"),
        "sudo apt install openfortivpn"
    );
    assert_eq!(
        install_command("linuxmint", "ubuntu debian"),
        "sudo apt install openfortivpn"
    );
}

#[test]
fn other_families() {
    assert_eq!(
        install_command("fedora", ""),
        "sudo dnf install openfortivpn"
    );
    assert_eq!(
        install_command("opensuse-leap", "suse"),
        "sudo zypper install openfortivpn"
    );
    assert_eq!(install_command("alpine", ""), "sudo apk add openfortivpn");
    assert_eq!(
        install_command("gentoo", ""),
        "sudo emerge --ask net-vpn/openfortivpn"
    );
}

#[test]
fn unknown_falls_back_to_source() {
    let c = install_command("plan9", "");
    assert!(c.contains("github.com/adrienverge/openfortivpn"));
}

#[test]
fn sudoers_tip_content() {
    assert_eq!(SUDOERS_EDIT_CMD, "sudo EDITOR=nano visudo");
    assert_eq!(
        sudoers_rule("joao", "/usr/bin/openfortivpn"),
        "joao ALL=(root) NOPASSWD: /usr/bin/openfortivpn"
    );
}
