//! AC7: Determinism: `diff`/`report` output is stable across runs for fixed inputs;
//! no wall-clock leaks into the classification.

use mqo_replay::diff::{DiffConfig, diff_logs};
use mqo_replay::record::DecisionRecord;
use mqo_replay::report::{ReportConfig, generate_report};

fn make_records() -> Vec<DecisionRecord> {
    vec![
        DecisionRecord {
            ts: "2026-01-01T00:00:00Z".to_string(),
            session: "s1".to_string(),
            question: "what is revenue?".to_string(),
            plan: vec![],
            access_verdict: "allow".to_string(),
            budget_consumed: 1.0,
            pillars_fired: vec!["pillar_measure".to_string()],
            outcome: "answered".to_string(),
            credential_id: None,
            answer_value: Some(1000.0),
            bound_measure: Some("revenue".to_string()),
            bound_grain: Some("month".to_string()),
        },
        DecisionRecord {
            ts: "2026-01-01T00:01:00Z".to_string(),
            session: "s1".to_string(),
            question: "what is margin?".to_string(),
            plan: vec![],
            access_verdict: "allow".to_string(),
            budget_consumed: 1.0,
            pillars_fired: vec!["pillar_measure".to_string(), "pillar_grain".to_string()],
            outcome: "answered".to_string(),
            credential_id: None,
            answer_value: Some(250.5),
            bound_measure: Some("margin".to_string()),
            bound_grain: Some("quarter".to_string()),
        },
    ]
}

#[test]
fn test_determinism() {
    let baseline = make_records();

    // Fresh has one drift
    let mut fresh = baseline.clone();
    fresh[1].outcome = "clarify".to_string();

    let config = DiffConfig::default();

    // Run diff twice
    let rows1 = diff_logs(&baseline, &fresh, &config);
    let rows2 = diff_logs(&baseline, &fresh, &config);

    // Serialize both outputs and compare
    let json1 = serde_json::to_string(&rows1).expect("serialize");
    let json2 = serde_json::to_string(&rows2).expect("serialize");
    assert_eq!(json1, json2, "diff output should be deterministic");

    // Report should also be deterministic
    let report_config = ReportConfig { fail_on: vec![] };
    let summary1 = generate_report(&rows1, &report_config);
    let summary2 = generate_report(&rows2, &report_config);

    let rs1 = serde_json::to_string(&summary1).expect("serialize");
    let rs2 = serde_json::to_string(&summary2).expect("serialize");
    assert_eq!(rs1, rs2, "report output should be deterministic");

    // Classes should not contain any timestamp or wall-clock fields
    for row in &rows1 {
        let json = serde_json::to_string(&row.class).expect("serialize class");
        // Class is just an enum — should not contain time
        assert!(
            !json.contains("2026") && !json.contains("clock"),
            "drift class should not contain timestamps: {}",
            json
        );
    }
}

#[test]
fn test_diff_order_matches_baseline_order() {
    // Output order should follow baseline order, not be sorted by question or timestamp
    let baseline = make_records();
    let fresh = baseline.clone();
    let config = DiffConfig::default();
    let rows = diff_logs(&baseline, &fresh, &config);

    assert_eq!(rows.len(), baseline.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.question, baseline[i].question,
            "row {} question should match baseline order", i);
    }
}
