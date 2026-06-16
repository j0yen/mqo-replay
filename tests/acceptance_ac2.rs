//! AC2: `diff` against a no-change fixture agent classifies every item `unchanged`.

use mqo_replay::diff::{DiffClass, DiffConfig, diff_logs};
use mqo_replay::record::{DecisionRecord, parse_log};

fn fixture_records() -> Vec<DecisionRecord> {
    let log = r#"
{"ts":"2026-01-01T00:00:00Z","session":"s1","question":"what is revenue?","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered","pillars_fired":["pillar_measure"],"bound_measure":"revenue","bound_grain":"month","answer_value":1000.0}
{"ts":"2026-01-01T00:01:00Z","session":"s1","question":"what is margin?","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered","pillars_fired":["pillar_measure"],"bound_measure":"margin","bound_grain":"quarter","answer_value":250.0}
"#;
    parse_log(log).expect("should parse")
}

#[test]
fn test_diff_no_change_all_unchanged() {
    let baseline = fixture_records();
    // "fresh" is identical to baseline
    let fresh = baseline.clone();

    let config = DiffConfig::default();
    let rows = diff_logs(&baseline, &fresh, &config);

    assert_eq!(rows.len(), baseline.len(), "should have one row per question");
    for row in &rows {
        assert_eq!(
            row.class,
            DiffClass::Unchanged,
            "no-change diff should classify as unchanged for question: {}",
            row.question
        );
    }
}
