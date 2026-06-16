//! AC4: `value_drift` fires only when the numeric answer moves beyond `--value-tol`
//! and not within it (boundary tested at the tolerance edge).

use mqo_replay::diff::{DiffClass, DiffConfig, classify};
use mqo_replay::record::DecisionRecord;

fn rec_with_value(value: Option<f64>) -> DecisionRecord {
    DecisionRecord {
        ts: "2026-01-01T00:00:00Z".to_string(),
        session: "s1".to_string(),
        question: "what is revenue?".to_string(),
        plan: vec![],
        access_verdict: "allow".to_string(),
        budget_consumed: 1.0,
        pillars_fired: vec![],
        outcome: "answered".to_string(),
        credential_id: None,
        answer_value: value,
        bound_measure: None,
        bound_grain: None,
    }
}

#[test]
fn test_value_drift_boundary() {
    let tol = 0.5_f64;
    let config = DiffConfig { value_tol: tol, relative: false };

    let base = rec_with_value(Some(100.0));

    // Exactly at tolerance: should be unchanged (≤ tol)
    let at_tol = rec_with_value(Some(100.0 + tol));
    assert_eq!(
        classify(&base, &at_tol, &config),
        DiffClass::Unchanged,
        "exactly at tolerance boundary should be unchanged"
    );

    // Just beyond tolerance: should be value_drift
    let beyond_tol = rec_with_value(Some(100.0 + tol + f64::EPSILON * 1000.0));
    assert_eq!(
        classify(&base, &beyond_tol, &config),
        DiffClass::ValueDrift,
        "just beyond tolerance should be value_drift"
    );

    // Well within tolerance: unchanged
    let within = rec_with_value(Some(100.0 + tol * 0.1));
    assert_eq!(
        classify(&base, &within, &config),
        DiffClass::Unchanged,
        "well within tolerance should be unchanged"
    );

    // Negative direction: beyond tol
    let neg_beyond = rec_with_value(Some(100.0 - tol - f64::EPSILON * 1000.0));
    assert_eq!(
        classify(&base, &neg_beyond, &config),
        DiffClass::ValueDrift,
        "negative direction beyond tolerance should be value_drift"
    );

    // No value on either side: unchanged
    let no_val_base = rec_with_value(None);
    let no_val_fresh = rec_with_value(None);
    assert_eq!(
        classify(&no_val_base, &no_val_fresh, &config),
        DiffClass::Unchanged,
        "both None should be unchanged"
    );

    // Base has value, fresh doesn't: value_drift
    let fresh_no_val = rec_with_value(None);
    assert_eq!(
        classify(&base, &fresh_no_val, &config),
        DiffClass::ValueDrift,
        "base has value, fresh doesn't: should be value_drift"
    );
}
