//! AC5: `report --fail-on bind_drift,value_drift` exits non-zero when those classes
//! are present and zero when only `plan_drift` is present.

use mqo_replay::diff::{DiffClass, DiffRow};
use mqo_replay::record::DecisionRecord;
use mqo_replay::report::{ReportConfig, generate_report};

fn make_row(question: &str, class: DiffClass) -> DiffRow {
    let rec = DecisionRecord {
        ts: "2026-01-01T00:00:00Z".to_string(),
        session: "s1".to_string(),
        question: question.to_string(),
        plan: vec![],
        access_verdict: "allow".to_string(),
        budget_consumed: 1.0,
        pillars_fired: vec![],
        outcome: "answered".to_string(),
        credential_id: None,
        answer_value: None,
        bound_measure: None,
        bound_grain: None,
    };
    DiffRow {
        question: question.to_string(),
        class,
        baseline: rec.clone(),
        replay: rec,
    }
}

#[test]
fn test_report_fail_on_exit_codes() {
    let fail_on = ReportConfig::parse_fail_on("bind_drift,value_drift").expect("parse fail_on");

    // Case 1: bind_drift present → should_fail = true
    let rows_bind = vec![make_row("q?", DiffClass::BindDrift)];
    let config = ReportConfig { fail_on: fail_on.clone() };
    let summary = generate_report(&rows_bind, &config);
    assert!(summary.should_fail, "bind_drift should trigger failure");
    assert!(
        summary.failing_classes.contains(&"bind_drift".to_string()),
        "failing_classes should include bind_drift"
    );

    // Case 2: value_drift present → should_fail = true
    let rows_value = vec![make_row("q?", DiffClass::ValueDrift)];
    let config = ReportConfig { fail_on: fail_on.clone() };
    let summary = generate_report(&rows_value, &config);
    assert!(summary.should_fail, "value_drift should trigger failure");

    // Case 3: only plan_drift present → should_fail = false
    let rows_plan = vec![make_row("q?", DiffClass::PlanDrift)];
    let config = ReportConfig { fail_on: fail_on.clone() };
    let summary = generate_report(&rows_plan, &config);
    assert!(!summary.should_fail, "plan_drift alone should not trigger failure");

    // Case 4: no drift at all → should_fail = false
    let rows_clean = vec![make_row("q?", DiffClass::Unchanged)];
    let config = ReportConfig { fail_on };
    let summary = generate_report(&rows_clean, &config);
    assert!(!summary.should_fail, "unchanged should not trigger failure");

    // Case 5: both bind_drift and value_drift present
    let rows_both = vec![
        make_row("q1?", DiffClass::BindDrift),
        make_row("q2?", DiffClass::ValueDrift),
    ];
    let fail_on2 = ReportConfig::parse_fail_on("bind_drift,value_drift").expect("parse");
    let config = ReportConfig { fail_on: fail_on2 };
    let summary = generate_report(&rows_both, &config);
    assert!(summary.should_fail, "both classes present should trigger failure");
    assert_eq!(summary.failing_classes.len(), 2, "both classes should be in failing_classes");
}
