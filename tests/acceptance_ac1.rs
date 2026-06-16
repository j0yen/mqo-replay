//! AC1: `run` extracts every question from a fixture log and produces one fresh
//! record per question via the (mocked) agent subprocess.

use mqo_replay::record::parse_log;
use mqo_replay::replay::{ReplayConfig, run_replay};

const FIXTURE_LOG: &str = r#"
{"ts":"2026-01-01T00:00:00Z","session":"s1","question":"what is revenue?","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered"}
{"ts":"2026-01-01T00:01:00Z","session":"s1","question":"what is margin?","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered"}
{"ts":"2026-01-01T00:02:00Z","session":"s1","question":"show me costs","access_verdict":"allow","budget_consumed":1.0,"outcome":"clarify"}
"#;

// Mock agent: echoes a valid DecisionRecord for any input
fn no_change_agent_cmd() -> String {
    // sh -c with inline script: reads question from env, emits a fixed record
    r#"echo '{"ts":"2026-01-01T00:00:00Z","session":"replay","question":"x","access_verdict":"allow","budget_consumed":1.0,"outcome":"answered"}'"#.to_string()
}

#[test]
fn test_run_produces_one_record_per_question() {
    let baseline = parse_log(FIXTURE_LOG).expect("should parse fixture");
    assert_eq!(baseline.len(), 3, "fixture has 3 questions");

    let config = ReplayConfig {
        agent_cmd: no_change_agent_cmd(),
        since: None,
    };

    let replays = run_replay(&baseline, &config).expect("run should succeed");
    assert_eq!(replays.len(), 3, "should produce one record per question");

    // Each replay record should have the baseline question preserved
    for (i, r) in replays.iter().enumerate() {
        assert_eq!(
            r.baseline.question, baseline[i].question,
            "baseline question should be preserved at index {}", i
        );
        // fresh record is a valid DecisionRecord (parse succeeded)
        assert!(!r.fresh.outcome.is_empty(), "fresh record should have an outcome");
    }
}

#[test]
fn test_run_since_filter_reduces_count() {
    let baseline = parse_log(FIXTURE_LOG).expect("should parse fixture");

    let config = ReplayConfig {
        agent_cmd: no_change_agent_cmd(),
        // Only records at or after the second entry
        since: Some("2026-01-01T00:01:00Z".to_string()),
    };

    let replays = run_replay(&baseline, &config).expect("run should succeed");
    assert_eq!(replays.len(), 2, "since filter should exclude the first record");
}
