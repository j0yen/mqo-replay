//! Decision record type, mirroring mqo-decision-log's schema.
//!
//! We intentionally do NOT depend on mqo-decision-log as a crate —
//! we are an independent observer and must be independently deployable.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// The canonical decision record schema (matches mqo-decision-log).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionRecord {
    /// ISO-8601 / RFC-3339 timestamp
    pub ts: String,
    /// Session identifier
    pub session: String,
    /// The question that was posed
    pub question: String,
    /// Ordered list of plan steps
    #[serde(default)]
    pub plan: Vec<String>,
    /// Access policy verdict
    pub access_verdict: String,
    /// Budget consumed (arbitrary unit)
    pub budget_consumed: f64,
    /// Pillars that fired during this decision
    #[serde(default)]
    pub pillars_fired: Vec<String>,
    /// Outcome: "answered" | "clarify" | "blocked"
    pub outcome: String,
    /// Optional credential id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    /// Optional numeric answer value (for value_drift detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_value: Option<f64>,
    /// Optional bound measure name (for bind_drift detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_measure: Option<String>,
    /// Optional bound grain (for bind_drift detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_grain: Option<String>,
}

impl DecisionRecord {
    /// Validate that this record has the required fields.
    pub fn validate(&self) -> Result<()> {
        if self.ts.is_empty() {
            bail!("record missing required field: ts");
        }
        if self.question.is_empty() {
            bail!("record missing required field: question");
        }
        if !matches!(self.outcome.as_str(), "answered" | "clarify" | "blocked") {
            bail!(
                "record.outcome must be one of: answered, clarify, blocked (got {:?})",
                self.outcome
            );
        }
        Ok(())
    }
}

/// Parse a JSONL log file into a vector of records.
pub fn parse_log(content: &str) -> Result<Vec<DecisionRecord>> {
    let mut records = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: DecisionRecord = serde_json::from_str(trimmed)
            .map_err(|e| anyhow::anyhow!("line {}: {}", line_num + 1, e))?;
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_record() {
        let json = r#"{"ts":"2026-01-01T00:00:00Z","session":"s1","question":"q?","plan":[],"access_verdict":"allow","budget_consumed":1.0,"pillars_fired":[],"outcome":"answered"}"#;
        let records = parse_log(json).expect("should parse");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].question, "q?");
    }

    #[test]
    fn test_parse_invalid_outcome_fails() {
        let json = r#"{"ts":"2026-01-01T00:00:00Z","session":"s1","question":"q?","plan":[],"access_verdict":"allow","budget_consumed":1.0,"pillars_fired":[],"outcome":"unknown"}"#;
        assert!(parse_log(json).is_err());
    }

    #[test]
    fn test_parse_multiple_lines() {
        let json = "
{\"ts\":\"2026-01-01T00:00:00Z\",\"session\":\"s1\",\"question\":\"q1?\",\"access_verdict\":\"allow\",\"budget_consumed\":1.0,\"outcome\":\"answered\"}
{\"ts\":\"2026-01-01T00:01:00Z\",\"session\":\"s1\",\"question\":\"q2?\",\"access_verdict\":\"allow\",\"budget_consumed\":1.0,\"outcome\":\"clarify\"}
";
        let records = parse_log(json).expect("should parse");
        assert_eq!(records.len(), 2);
    }
}
