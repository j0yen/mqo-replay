//! `run` subcommand: invoke the agent subprocess for each logged question.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::record::DecisionRecord;

/// Configuration for a replay run.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Command to invoke as the agent subprocess. Default: "mqo-agent".
    pub agent_cmd: String,
    /// Optional: only replay records at or after this ISO-8601 timestamp.
    pub since: Option<String>,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            agent_cmd: "mqo-agent".to_string(),
            since: None,
        }
    }
}

/// A fresh record produced by replaying a logged question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// The original logged record (baseline).
    pub baseline: DecisionRecord,
    /// The fresh record from replaying through the current agent.
    pub fresh: DecisionRecord,
}

/// Run the agent subprocess with a question, returning the parsed DecisionRecord.
///
/// Protocol: question is passed as the first positional argument AND via stdin.
/// The agent must emit a JSON DecisionRecord on stdout.
pub fn invoke_agent(agent_cmd: &str, question: &str) -> Result<DecisionRecord> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(agent_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MQO_REPLAY_QUESTION", question)
        .spawn()
        .with_context(|| format!("failed to spawn agent: {}", agent_cmd))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(question.as_bytes())
            .context("failed to write question to agent stdin")?;
        // Drop stdin to signal EOF
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for agent")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "agent exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let record: DecisionRecord = serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse agent output as DecisionRecord: {}", stdout.trim()))?;

    Ok(record)
}

/// Apply the since filter: keep records at or after the given ISO timestamp.
fn passes_since_filter(record: &DecisionRecord, since: &Option<String>) -> bool {
    match since {
        None => true,
        Some(since_ts) => record.ts.as_str() >= since_ts.as_str(),
    }
}

/// Replay all (or filtered) records from the baseline log through the agent.
pub fn run_replay(
    baseline: &[DecisionRecord],
    config: &ReplayConfig,
) -> Result<Vec<ReplayRecord>> {
    let mut results = Vec::new();
    for record in baseline {
        if !passes_since_filter(record, &config.since) {
            continue;
        }
        let fresh = invoke_agent(&config.agent_cmd, &record.question)
            .with_context(|| format!("replaying question: {}", record.question))?;
        results.push(ReplayRecord {
            baseline: record.clone(),
            fresh,
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::DecisionRecord;

    fn make_record(question: &str, outcome: &str) -> DecisionRecord {
        DecisionRecord {
            ts: "2026-01-01T00:00:00Z".to_string(),
            session: "s1".to_string(),
            question: question.to_string(),
            plan: vec![],
            access_verdict: "allow".to_string(),
            budget_consumed: 1.0,
            pillars_fired: vec![],
            outcome: outcome.to_string(),
            credential_id: None,
            answer_value: None,
            bound_measure: None,
            bound_grain: None,
        }
    }

    #[test]
    fn test_since_filter_passes_equal() {
        let r = make_record("q?", "answered");
        let since = Some("2026-01-01T00:00:00Z".to_string());
        assert!(passes_since_filter(&r, &since));
    }

    #[test]
    fn test_since_filter_rejects_earlier() {
        let r = make_record("q?", "answered");
        let since = Some("2026-01-02T00:00:00Z".to_string());
        assert!(!passes_since_filter(&r, &since));
    }

    #[test]
    fn test_since_filter_none_passes_all() {
        let r = make_record("q?", "answered");
        assert!(passes_since_filter(&r, &None));
    }

    #[test]
    fn test_invoke_agent_echo() {
        // Use a shell command that echoes a valid DecisionRecord as the "agent"
        let question = "what is revenue?";
        let agent_cmd = r#"echo '{"ts":"2026-01-01T00:00:00Z","session":"s1","question":"what is revenue?","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered"}'"#;
        let record = invoke_agent(agent_cmd, question).expect("should succeed");
        assert_eq!(record.outcome, "answered");
    }
}
