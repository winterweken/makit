//! What the engine's tests share: murb's own numbers and the synthetic weather.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde_json::Value;

use super::Monthly;

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("energy")
        .join("testdata")
        .join(name)
}

/// `testdata/parity.json`: what murb's Python computes on the synthetic weather.
pub fn parity() -> &'static Value {
    static PARITY: OnceLock<Value> = OnceLock::new();
    PARITY.get_or_init(|| {
        let text = std::fs::read_to_string(testdata("parity.json")).expect("parity.json");
        serde_json::from_str(&text).expect("parity.json is JSON")
    })
}

/// The synthetic weather file the fixture was made on.
pub fn synthetic_epw() -> PathBuf {
    testdata("synthetic.epw")
}

/// Whether `got` is `want` to the port's tolerance.
pub fn close(got: f64, want: f64) -> bool {
    (got - want).abs() <= 1e-9 + 1e-9 * want.abs()
}

pub fn assert_close(label: &str, got: f64, want: f64) {
    assert!(close(got, want), "{label}: got {got}, murb gives {want}");
}

pub fn assert_monthly(label: &str, got: &Monthly, want: &Value) {
    let want = want
        .as_array()
        .unwrap_or_else(|| panic!("{label}: not a list in parity.json"));
    assert_eq!(want.len(), 12, "{label}: murb gives {} months", want.len());
    for (m, (g, w)) in got.iter().zip(want).enumerate() {
        let w = w
            .as_f64()
            .unwrap_or_else(|| panic!("{label}: not a number"));
        assert!(
            close(*g, w),
            "{label}, month {}: got {g}, murb gives {w}",
            m + 1
        );
    }
}
