//! `report` subcommand: summarize diff output and exit non-zero on specified classes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diff::{DiffClass, DiffRow};

/// Configuration for the report subcommand.
#[derive(Debug, Clone, Default)]
pub struct ReportConfig {
    /// Drift classes that should cause a non-zero exit.
    pub fail_on: Vec<DiffClass>,
}

impl ReportConfig {
    /// Parse a comma-separated list of class names.
    pub fn parse_fail_on(s: &str) -> anyhow::Result<Vec<DiffClass>> {
        if s.is_empty() {
            return Ok(vec![]);
        }
        s.split(',')
            .map(|part| match part.trim() {
                "unchanged" => Ok(DiffClass::Unchanged),
                "plan_drift" => Ok(DiffClass::PlanDrift),
                "bind_drift" => Ok(DiffClass::BindDrift),
                "outcome_drift" => Ok(DiffClass::OutcomeDrift),
                "value_drift" => Ok(DiffClass::ValueDrift),
                other => anyhow::bail!("unknown drift class: {}", other),
            })
            .collect()
    }
}

/// Summary of a diff run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total questions replayed.
    pub total: usize,
    /// Counts per drift class (sorted for deterministic serialization).
    pub counts: BTreeMap<String, usize>,
    /// The worst offenders (first 10 drifted questions with their class).
    pub worst: Vec<WorstOffender>,
    /// Whether the report should exit non-zero (based on fail_on config).
    pub should_fail: bool,
    /// Which classes triggered the failure.
    pub failing_classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorstOffender {
    pub question: String,
    pub class: String,
}

/// Generate a report summary from diff rows.
pub fn generate_report(rows: &[DiffRow], config: &ReportConfig) -> ReportSummary {
    let total = rows.len();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for row in rows {
        *counts.entry(row.class.to_string()).or_insert(0) += 1;
    }

    // Worst offenders: non-unchanged items, up to 10
    let worst: Vec<WorstOffender> = rows
        .iter()
        .filter(|r| r.class != DiffClass::Unchanged)
        .take(10)
        .map(|r| WorstOffender {
            question: r.question.clone(),
            class: r.class.to_string(),
        })
        .collect();

    // Determine if any fail_on class is present
    let failing_classes: Vec<String> = config
        .fail_on
        .iter()
        .filter(|cls| counts.get(&cls.to_string()).copied().unwrap_or(0) > 0)
        .map(|cls| cls.to_string())
        .collect();

    let should_fail = !failing_classes.is_empty();

    ReportSummary {
        total,
        counts,
        worst,
        should_fail,
        failing_classes,
    }
}

/// Render a human-readable report to a string.
pub fn render_report(summary: &ReportSummary) -> String {
    let mut out = String::new();
    out.push_str(&format!("mqo-replay report: {} questions\n", summary.total));
    out.push_str("─────────────────────────────────\n");

    let mut classes = vec![
        "unchanged",
        "plan_drift",
        "bind_drift",
        "outcome_drift",
        "value_drift",
    ];
    classes.sort_unstable();

    for cls in &classes {
        let count = summary.counts.get(*cls).copied().unwrap_or(0);
        if count > 0 || *cls == "unchanged" {
            out.push_str(&format!("  {:15} {}\n", cls, count));
        }
    }

    if !summary.worst.is_empty() {
        out.push_str("\nWorst offenders:\n");
        for w in &summary.worst {
            out.push_str(&format!("  [{:13}] {}\n", w.class, w.question));
        }
    }

    if summary.should_fail {
        out.push_str(&format!(
            "\nFAIL: drift classes present: {}\n",
            summary.failing_classes.join(", ")
        ));
    } else {
        out.push_str("\nPASS\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffClass;
    use crate::record::DecisionRecord;

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
    fn test_report_fail_on_bind_drift_when_present() {
        let rows = vec![
            make_row("q1?", DiffClass::BindDrift),
            make_row("q2?", DiffClass::PlanDrift),
        ];
        let config = ReportConfig {
            fail_on: vec![DiffClass::BindDrift, DiffClass::ValueDrift],
        };
        let summary = generate_report(&rows, &config);
        assert!(summary.should_fail);
        assert!(summary.failing_classes.contains(&"bind_drift".to_string()));
    }

    #[test]
    fn test_report_no_fail_on_plan_drift_only() {
        let rows = vec![make_row("q1?", DiffClass::PlanDrift)];
        let config = ReportConfig {
            fail_on: vec![DiffClass::BindDrift, DiffClass::ValueDrift],
        };
        let summary = generate_report(&rows, &config);
        assert!(!summary.should_fail);
    }

    #[test]
    fn test_report_counts_correctly() {
        let rows = vec![
            make_row("q1?", DiffClass::Unchanged),
            make_row("q2?", DiffClass::Unchanged),
            make_row("q3?", DiffClass::PlanDrift),
        ];
        let config = ReportConfig::default();
        let summary = generate_report(&rows, &config);
        assert_eq!(*summary.counts.get("unchanged").unwrap(), 2);
        assert_eq!(*summary.counts.get("plan_drift").unwrap(), 1);
        assert_eq!(summary.total, 3);
    }

    #[test]
    fn test_parse_fail_on() {
        let classes = ReportConfig::parse_fail_on("bind_drift,value_drift").expect("should parse");
        assert_eq!(classes.len(), 2);
        assert!(classes.contains(&DiffClass::BindDrift));
        assert!(classes.contains(&DiffClass::ValueDrift));
    }

    #[test]
    fn test_parse_fail_on_unknown() {
        assert!(ReportConfig::parse_fail_on("unknown_class").is_err());
    }
}
