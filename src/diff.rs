//! `diff` subcommand: classify deltas between baseline and fresh records.
//!
//! Drift categories (in priority order — first match wins):
//!   `outcome_drift` — outcome field changed (answered/clarify/blocked)
//!   `bind_drift`    — bound_measure or bound_grain changed
//!   `plan_drift`    — pillars_fired set changed (regardless of order)
//!   `value_drift`   — numeric answer_value changed beyond tolerance
//!   `unchanged`     — none of the above

use serde::{Deserialize, Serialize};

use crate::record::DecisionRecord;
use crate::replay::ReplayRecord;

/// The tolerance convention for numeric comparison, matching mqo-engine-parity.
/// Default: absolute tolerance 1e-6.
#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Absolute tolerance for value_drift (default 1e-6).
    pub value_tol: f64,
    /// When true, use relative tolerance instead of absolute.
    pub relative: bool,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            value_tol: 1e-6,
            relative: false,
        }
    }
}

/// Drift classification for a single question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffClass {
    Unchanged,
    PlanDrift,
    BindDrift,
    OutcomeDrift,
    ValueDrift,
}

impl std::fmt::Display for DiffClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffClass::Unchanged => write!(f, "unchanged"),
            DiffClass::PlanDrift => write!(f, "plan_drift"),
            DiffClass::BindDrift => write!(f, "bind_drift"),
            DiffClass::OutcomeDrift => write!(f, "outcome_drift"),
            DiffClass::ValueDrift => write!(f, "value_drift"),
        }
    }
}

/// One row of diff output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffRow {
    /// The question that was replayed.
    pub question: String,
    /// The drift classification.
    pub class: DiffClass,
    /// The baseline record.
    pub baseline: DecisionRecord,
    /// The fresh record from replay.
    pub replay: DecisionRecord,
}

/// Compare two sets of sorted pillars for equality.
fn pillars_equal(a: &[String], b: &[String]) -> bool {
    let mut sa: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
    let mut sb: Vec<&str> = b.iter().map(|s| s.as_str()).collect();
    sa.sort_unstable();
    sb.sort_unstable();
    sa == sb
}

/// Numeric comparison matching mqo-engine-parity's convention.
/// Returns true if the two values are within tolerance.
pub fn values_within_tolerance(a: f64, b: f64, tol: f64, relative: bool) -> bool {
    if relative {
        let denom = a.abs().max(b.abs());
        if denom == 0.0 {
            return true; // both zero
        }
        (a - b).abs() / denom <= tol
    } else {
        (a - b).abs() <= tol
    }
}

/// Classify the drift between a baseline and fresh record pair.
pub fn classify(baseline: &DecisionRecord, fresh: &DecisionRecord, config: &DiffConfig) -> DiffClass {
    // Priority: outcome_drift > bind_drift > plan_drift > value_drift > unchanged

    // 1. outcome_drift
    if baseline.outcome != fresh.outcome {
        return DiffClass::OutcomeDrift;
    }

    // 2. bind_drift (bound_measure or bound_grain changed)
    if baseline.bound_measure != fresh.bound_measure || baseline.bound_grain != fresh.bound_grain {
        return DiffClass::BindDrift;
    }

    // 3. plan_drift (pillars_fired set changed)
    if !pillars_equal(&baseline.pillars_fired, &fresh.pillars_fired) {
        return DiffClass::PlanDrift;
    }

    // 4. value_drift (numeric answer moved beyond tolerance)
    match (baseline.answer_value, fresh.answer_value) {
        (Some(a), Some(b)) => {
            if !values_within_tolerance(a, b, config.value_tol, config.relative) {
                return DiffClass::ValueDrift;
            }
        }
        (None, None) => {}
        // One side has a value and the other doesn't — treat as value_drift
        _ => return DiffClass::ValueDrift,
    }

    DiffClass::Unchanged
}

/// Diff a set of replay records, returning one DiffRow per question.
pub fn diff_records(replays: &[ReplayRecord], config: &DiffConfig) -> Vec<DiffRow> {
    replays
        .iter()
        .map(|r| {
            let class = classify(&r.baseline, &r.fresh, config);
            DiffRow {
                question: r.baseline.question.clone(),
                class,
                baseline: r.baseline.clone(),
                replay: r.fresh.clone(),
            }
        })
        .collect()
}

/// Diff a baseline log against a fresh log by matching on question text.
pub fn diff_logs(
    baseline: &[DecisionRecord],
    fresh: &[DecisionRecord],
    config: &DiffConfig,
) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for base_rec in baseline {
        // Find the matching fresh record by question
        if let Some(fresh_rec) = fresh.iter().find(|r| r.question == base_rec.question) {
            let class = classify(base_rec, fresh_rec, config);
            rows.push(DiffRow {
                question: base_rec.question.clone(),
                class,
                baseline: base_rec.clone(),
                replay: fresh_rec.clone(),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::DecisionRecord;

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
    fn test_unchanged() {
        let b = rec("q?", "answered", &["pillar_a"], Some("revenue"), Some("month"), Some(100.0));
        let f = b.clone();
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::Unchanged);
    }

    #[test]
    fn test_outcome_drift() {
        let b = rec("q?", "answered", &["pillar_a"], None, None, None);
        let f = rec("q?", "clarify", &["pillar_a"], None, None, None);
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::OutcomeDrift);
    }

    #[test]
    fn test_bind_drift_measure() {
        let b = rec("q?", "answered", &["pillar_a"], Some("revenue"), None, None);
        let f = rec("q?", "answered", &["pillar_a"], Some("margin"), None, None);
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::BindDrift);
    }

    #[test]
    fn test_plan_drift() {
        let b = rec("q?", "answered", &["pillar_a"], None, None, None);
        let f = rec("q?", "answered", &["pillar_b"], None, None, None);
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::PlanDrift);
    }

    #[test]
    fn test_value_drift_beyond_tolerance() {
        let b = rec("q?", "answered", &[], None, None, Some(100.0));
        let f = rec("q?", "answered", &[], None, None, Some(101.0));
        let config = DiffConfig { value_tol: 0.5, relative: false };
        let class = classify(&b, &f, &config);
        assert_eq!(class, DiffClass::ValueDrift);
    }

    #[test]
    fn test_value_within_tolerance() {
        let b = rec("q?", "answered", &[], None, None, Some(100.0));
        let f = rec("q?", "answered", &[], None, None, Some(100.0000001));
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::Unchanged);
    }

    #[test]
    fn test_value_drift_boundary_exact() {
        // At exactly tol: within (not drift)
        let b = rec("q?", "answered", &[], None, None, Some(100.0));
        let f = rec("q?", "answered", &[], None, None, Some(100.5));
        let config = DiffConfig { value_tol: 0.5, relative: false };
        let class = classify(&b, &f, &config);
        assert_eq!(class, DiffClass::Unchanged, "exactly at tolerance should be unchanged");
    }

    #[test]
    fn test_value_drift_boundary_beyond() {
        // Just beyond tol: drift
        let b = rec("q?", "answered", &[], None, None, Some(100.0));
        let f = rec("q?", "answered", &[], None, None, Some(100.500001));
        let config = DiffConfig { value_tol: 0.5, relative: false };
        let class = classify(&b, &f, &config);
        assert_eq!(class, DiffClass::ValueDrift, "just beyond tolerance should be value_drift");
    }

    #[test]
    fn test_relative_tolerance() {
        // 1% relative tolerance: 100.0 vs 101.0 = 1% diff (at boundary)
        let b = rec("q?", "answered", &[], None, None, Some(100.0));
        let f = rec("q?", "answered", &[], None, None, Some(101.0));
        let config = DiffConfig { value_tol: 0.01, relative: true };
        let class = classify(&b, &f, &config);
        assert_eq!(class, DiffClass::Unchanged, "1% diff at 1% relative tol = unchanged");
    }

    #[test]
    fn test_plan_drift_order_independent() {
        // Order of pillars_fired doesn't matter
        let b = rec("q?", "answered", &["a", "b", "c"], None, None, None);
        let f = rec("q?", "answered", &["c", "a", "b"], None, None, None);
        let class = classify(&b, &f, &DiffConfig::default());
        assert_eq!(class, DiffClass::Unchanged);
    }
}
