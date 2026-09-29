use openfortivpn_gui::model::state::{fmt_kb, fmt_mb};

#[test]
fn speed_formatting_does_not_divide_twice() {
    // Regression: the sampler emits KB/s; formatting must not divide again.
    // 1 MB/s ≈ 977 KB/s used to render as "1 KB/s".
    assert_eq!(fmt_kb(977.0), "977");
    assert_eq!(fmt_kb(1200.0), "1200");
    assert_eq!(fmt_kb(0.4), "0");
    assert_eq!(fmt_kb(25.6), "26");
}

#[test]
fn totals_formatting() {
    assert_eq!(fmt_mb(1_048_576), "1.0");
    assert_eq!(fmt_mb(12 * 1_048_576 + 300_000), "12.3");
    assert_eq!(fmt_mb(0), "0.0");
}
