//! AC3: `diff` against a drifted fixture agent correctly labels at least one each of
//! `plan_drift`, `bind_drift`, and `outcome_drift`, naming the question.

use mqo_replay::diff::{DiffClass, DiffConfig, diff_logs};
use mqo_replay::record::DecisionRecord;

fn rec(
    question: &str,
    outcome: &str,
    pillars: &[&str],
    measure: Option<&str>,
    grain: Option<&str>,
    value: Option<f64>,
) -> DecisionRecord {
    DecisionRecord {
        ts: "2026-01-01T00:00:00Z".to_string(),
        session: "s1".to_string(),
        question: question.to_string(),
        plan: vec![],
        access_verdict: "allow".to_string(),
        budget_consumed: 1.0,
        pillars_fired: pillars.iter().map(|s| s.to_string()).collect(),
        outcome: outcome.to_string(),
        credential_id: None,
        answer_value: value,
        bound_measure: measure.map(|s| s.to_string()),
        bound_grain: grain.map(|s| s.to_string()),
    }
}

#[test]
fn test_diff_drifted_labels_all_categories() {
    // Baseline: all answered, specific pillars/binds/values
    let baseline = vec![
        rec("q_outcome?", "answered", &["pillar_a"], None, None, None),
        rec("q_bind?", "answered", &["pillar_a"], Some("revenue"), Some("month"), None),
        rec("q_plan?", "answered", &["pillar_a"], None, None, None),
        rec("q_unchanged?", "answered", &["pillar_a"], Some("cost"), None, None),
    ];

    // Fresh (drifted): each question shows a different drift class
    let fresh = vec![
        // q_outcome: outcome changed to clarify → outcome_drift
        rec("q_outcome?", "clarify", &["pillar_a"], None, None, None),
        // q_bind: measure changed → bind_drift
        rec("q_bind?", "answered", &["pillar_a"], Some("margin"), Some("month"), None),
        // q_plan: different pillar fired → plan_drift
        rec("q_plan?", "answered", &["pillar_b"], None, None, None),
        // q_unchanged: identical → unchanged
        rec("q_unchanged?", "answered", &["pillar_a"], Some("cost"), None, None),
    ];

    let config = DiffConfig::default();
    let rows = diff_logs(&baseline, &fresh, &config);

    // Find each row by question
    let find = |q: &str| {
        rows.iter()
            .find(|r| r.question == q)
            .unwrap_or_else(|| panic!("missing row for {}", q))
    };

    assert_eq!(find("q_outcome?").class, DiffClass::OutcomeDrift, "q_outcome? should be outcome_drift");
    assert_eq!(find("q_bind?").class, DiffClass::BindDrift, "q_bind? should be bind_drift");
    assert_eq!(find("q_plan?").class, DiffClass::PlanDrift, "q_plan? should be plan_drift");
    assert_eq!(find("q_unchanged?").class, DiffClass::Unchanged, "q_unchanged? should be unchanged");

    // Verify questions are named in each drifted row
    assert_eq!(find("q_outcome?").question, "q_outcome?");
    assert_eq!(find("q_bind?").question, "q_bind?");
    assert_eq!(find("q_plan?").question, "q_plan?");
}
