//! AC6: Numeric comparison matches `mqo-engine-parity`'s tolerance convention:
//! absolute tolerance by default (1e-6), relative optional; same inputs yield same verdict.

use mqo_replay::diff::{DiffConfig, values_within_tolerance};

/// This mirrors the `values_match` function in mqo-engine-parity/src/lib.rs
/// for absolute and relative modes.
fn engine_parity_match(a: f64, b: f64, tol: f64, relative: bool) -> bool {
    if relative {
        let denom = a.abs().max(b.abs());
        if denom == 0.0 {
            return true;
        }
        (a - b).abs() / denom <= tol
    } else {
        (a - b).abs() <= tol
    }
}

/// Test that our values_within_tolerance matches mqo-engine-parity's convention
/// for the same inputs.
#[test]
fn test_numeric_tolerance_matches_engine_parity() {
    let test_cases: &[(f64, f64, f64, bool)] = &[
        // (a, b, tol, relative)
        (100.0, 100.0, 1e-6, false),        // identical: within
        (100.0, 100.0000001, 1e-6, false),  // tiny diff: within absolute
        (100.0, 100.001, 1e-6, false),      // beyond 1e-6 absolute
        (100.0, 100.001, 0.01, false),      // within 0.01 absolute
        (100.0, 101.0, 0.01, true),         // 1% diff at 1% relative: within
        (100.0, 102.0, 0.01, true),         // 2% diff at 1% relative: beyond
        (0.0, 0.0, 1e-6, false),            // both zero
        (0.0, 0.0, 1e-6, true),             // both zero relative
        (1000000.0, 1000001.0, 1e-6, true), // 1e-6 relative: within
        (-100.0, -101.0, 0.01, true),       // negative values relative
    ];

    for &(a, b, tol, relative) in test_cases {
        let our_result = values_within_tolerance(a, b, tol, relative);
        let parity_result = engine_parity_match(a, b, tol, relative);
        assert_eq!(
            our_result,
            parity_result,
            "mismatch for a={}, b={}, tol={}, relative={}: ours={}, parity={}",
            a, b, tol, relative, our_result, parity_result
        );
    }
}

#[test]
fn test_default_tolerance_is_1e6_absolute() {
    let config = DiffConfig::default();
    assert!((config.value_tol - 1e-6).abs() < 1e-15, "default tolerance should be 1e-6");
    assert!(!config.relative, "default should be absolute tolerance");
}

#[test]
fn test_same_inputs_same_verdict() {
    // Determinism: calling values_within_tolerance twice with the same args returns the same result
    let (a, b, tol, relative) = (100.0, 100.5, 0.5, false);
    let r1 = values_within_tolerance(a, b, tol, relative);
    let r2 = values_within_tolerance(a, b, tol, relative);
    assert_eq!(r1, r2, "should be deterministic");
}
