use openfortivpn_gui::controller::vpn::{build_args, extract_sha256, temp_config_contents};
use openfortivpn_gui::model::profile::Profile;
use std::path::Path;

fn sample_profile() -> Profile {
    let mut p = Profile::new("CETIQT");
    p.server = "vpn2.cetiqt.senai.br:10443".into();
    p.username = "joao.araujo".into();
    p.trust_cert = true;
    p.trusted_cert =
        "10adad89836d4dbb03570776d469473ddb5dd25ceb6b60826f174275d8c1cfd3".to_uppercase();
    p
}

#[test]
fn temp_config_has_all_keys_and_lowercased_cert() {
    let txt = temp_config_contents(&sample_profile(), "v3ry-s3cret");
    assert!(txt.contains("host = vpn2.cetiqt.senai.br\n"));
    assert!(txt.contains("port = 10443\n"));
    assert!(txt.contains("username = joao.araujo\n"));
    assert!(txt.contains("password = v3ry-s3cret\n"));
    assert!(txt.contains(
        "trusted-cert = 10adad89836d4dbb03570776d469473ddb5dd25ceb6b60826f174275d8c1cfd3\n"
    ));
}

#[test]
fn argv_never_contains_secrets() {
    let conf = Path::new("/tmp/ofg-x.conf");
    for passwordless in [true, false] {
        let args = build_args("/usr/bin/openfortivpn", conf, passwordless);
        assert_eq!(args[0], "sudo");
        assert!(args.contains(&"--config".to_owned()));
        // VPN password must never appear anywhere in argv...
        assert!(!args.iter().any(|a| a.contains("v3ry-s3cret")));
        // ...and sudo's password flag only shows in the non-passwordless variant.
        assert_eq!(args.contains(&"-S".to_owned()), !passwordless);
    }
}

#[test]
fn sha256_extraction_variants() {
    let h = "10adad89836d4dbb03570776d469473ddb5dd25ceb6b60826f174275d8c1cfd3";
    assert_eq!(
        extract_sha256(&format!("SHA256 fingerprint is {h}")),
        Some(h.into())
    );
    assert_eq!(
        extract_sha256(&format!("SHA256 fingerprint: {h}")),
        Some(h.into())
    );
    assert_eq!(extract_sha256("no digest here"), None);
    // Must not grab shorter hex runs.
    assert_eq!(extract_sha256("sha256 of deadbeef"), None);
}
