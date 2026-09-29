use openfortivpn_gui::model::profile::{Profile, ProfileBook};
use std::path::PathBuf;

fn tmp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ofg-test-{tag}-{}.json", std::process::id()))
}

#[test]
fn seed_default_matches_reference_command() {
    let b = ProfileBook::seed_default();
    assert_eq!(b.profiles.len(), 1);
    let p = &b.profiles[0];
    assert_eq!(p.name, "CETIQT");
    assert_eq!(p.host_port(), ("vpn2.cetiqt.senai.br".to_owned(), 10443));
    assert_eq!(p.username, "joao.araujo");
    assert!(p.trust_cert);
    assert_eq!(
        p.trusted_cert,
        "10adad89836d4dbb03570776d469473ddb5dd25ceb6b60826f174275d8c1cfd3"
    );
    assert_eq!(b.selected.as_deref(), Some(p.id.as_str()));
}

#[test]
fn host_port_parsing() {
    let mut p = Profile::new("x");
    p.server = "vpn.example.com:10443".into();
    assert_eq!(p.host_port(), ("vpn.example.com".to_owned(), 10443));
    p.server = "vpn.example.com".into();
    assert_eq!(p.host_port(), ("vpn.example.com".to_owned(), 443));
    p.server = " vpn.example.com:10443 ".into();
    assert_eq!(p.host_port(), ("vpn.example.com".to_owned(), 10443));
    p.server = "1.2.3.4:443".into();
    assert_eq!(p.host_port(), ("1.2.3.4".to_owned(), 443));
}

#[test]
fn validation_rules() {
    let mut p = Profile::new("");
    assert!(p.valid().is_err());
    p.name = "n".into();
    assert!(p.valid().is_err()); // no server
    p.server = "host:443".into();
    assert!(p.valid().is_err()); // no user
    p.username = "u".into();
    assert!(p.valid().is_ok());
    p.trust_cert = true;
    assert!(p.valid().is_err()); // trust without digest
    p.trusted_cert = "10ad".into();
    assert!(p.valid().is_err()); // too short
    p.trusted_cert = "A".repeat(64);
    assert!(p.valid().is_ok());
}

#[test]
fn serde_roundtrip_and_crud() {
    let mut b = ProfileBook::seed_default();
    let mut p2 = Profile::new("Second");
    p2.server = "vpn.corp.br".into();
    p2.username = "bob".into();
    b.upsert(p2.clone());
    assert_eq!(b.profiles.len(), 2);

    let path = tmp_path("roundtrip");
    b.save(&path).unwrap();
    let loaded = ProfileBook::load(&path).unwrap().expect("file exists");
    assert_eq!(loaded.profiles.len(), 2);
    assert_eq!(loaded.selected, b.selected);
    assert!(loaded.profiles.contains(&p2));
    std::fs::remove_file(&path).ok();

    assert!(b.delete(&p2.id));
    assert_eq!(b.profiles.len(), 1);
    assert_ne!(b.selected.as_deref(), Some(p2.id.as_str()));
}

#[test]
fn load_missing_file_is_none() {
    let path = std::env::temp_dir().join("ofg-test-does-not-exist.json");
    let _ = std::fs::remove_file(&path);
    assert!(ProfileBook::load(&path).unwrap().is_none());
}

#[test]
fn saved_file_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let path = tmp_path("perm");
    ProfileBook::seed_default().save(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    std::fs::remove_file(&path).ok();
}
