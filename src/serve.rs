//! `serve` subcommand: expose run/diff/report as MCP tool calls via stdin/stdout JSON-RPC.
//!
//! Protocol: reads JSON objects from stdin, one per line (newline-delimited JSON).
//! Each request: {"id": ..., "method": "run"|"diff"|"report", "params": {...}}
//! Each response: {"id": ..., "result": ...} or {"id": ..., "error": {"message": "..."}}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};

use crate::diff::{DiffConfig, diff_logs};
use crate::record::parse_log;
use crate::replay::{ReplayConfig, run_replay};
use crate::report::{ReportConfig, generate_report, render_report};

#[derive(Debug, Deserialize)]
struct Request {
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct Response {
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObj>,
}

#[derive(Debug, Serialize)]
struct ErrorObj {
    message: String,
}

fn handle_request(req: &Request) -> Result<Value> {
    match req.method.as_str() {
        "run" => {
            let log_content = req.params["log"].as_str().unwrap_or("").to_string();
            let agent_cmd = req.params["agent"]
                .as_str()
                .unwrap_or("mqo-agent")
                .to_string();
            let since = req.params["since"].as_str().map(|s| s.to_string());

            let baseline = parse_log(&log_content)?;
            let config = ReplayConfig { agent_cmd, since };
            let replays = run_replay(&baseline, &config)?;
            Ok(serde_json::to_value(&replays)?)
        }
        "diff" => {
            let baseline_content = req.params["baseline"].as_str().unwrap_or("").to_string();
            let replay_content = req.params["replay"].as_str().unwrap_or("").to_string();
            let value_tol = req.params["value_tol"].as_f64().unwrap_or(1e-6);
            let relative = req.params["relative"].as_bool().unwrap_or(false);

            let baseline = parse_log(&baseline_content)?;
            let fresh = parse_log(&replay_content)?;
            let config = DiffConfig { value_tol, relative };
            let rows = diff_logs(&baseline, &fresh, &config);
            Ok(serde_json::to_value(&rows)?)
        }
        "report" => {
            let diff_content = req.params["diff"].as_str().unwrap_or("").to_string();
            let fail_on_str = req.params["fail_on"].as_str().unwrap_or("").to_string();

            let rows: Vec<crate::diff::DiffRow> = serde_json::from_str(&diff_content)?;
            let fail_on = ReportConfig::parse_fail_on(&fail_on_str)?;
            let config = ReportConfig { fail_on };
            let summary = generate_report(&rows, &config);
            let rendered = render_report(&summary);
            Ok(serde_json::json!({
                "summary": summary,
                "rendered": rendered,
            }))
        }
        other => anyhow::bail!("unknown method: {}", other),
    }
}

/// Run the serve loop: read JSON-RPC requests from stdin, write responses to stdout.
pub fn serve_loop<R: BufRead, W: Write>(reader: R, mut writer: W) -> Result<()> {
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(trimmed) {
            Err(e) => Response {
                id: Value::Null,
                result: None,
                error: Some(ErrorObj {
                    message: format!("parse error: {}", e),
                }),
            },
            Ok(req) => {
                let id = req.id.clone();
                match handle_request(&req) {
                    Ok(result) => Response {
                        id,
                        result: Some(result),
                        error: None,
                    },
                    Err(e) => Response {
                        id,
                        result: None,
                        error: Some(ErrorObj {
                            message: e.to_string(),
                        }),
                    },
                }
            }
        };

        let json = serde_json::to_string(&response)?;
        writeln!(writer, "{}", json)?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn run_serve(input: &str) -> String {
        let reader = BufReader::new(input.as_bytes());
        let mut output = Vec::new();
        serve_loop(reader, &mut output).expect("serve loop should not fail");
        String::from_utf8(output).expect("valid utf8")
    }

    #[test]
    fn test_serve_unknown_method() {
        let input = r#"{"id":1,"method":"bogus","params":{}}"#;
        let output = run_serve(input);
        let resp: serde_json::Value = serde_json::from_str(output.trim()).expect("valid json");
        assert!(resp["error"]["message"].as_str().unwrap().contains("unknown method"));
    }

    #[test]
    fn test_serve_diff_empty_logs() {
        let input = r#"{"id":2,"method":"diff","params":{"baseline":"","replay":"","value_tol":0.5}}"#;
        let output = run_serve(input);
        let resp: serde_json::Value = serde_json::from_str(output.trim()).expect("valid json");
        assert!(resp["result"].is_array());
        assert_eq!(resp["result"].as_array().expect("array").len(), 0);
    }

    #[test]
    fn test_serve_report_no_fail() {
        let diff_rows: Vec<crate::diff::DiffRow> = vec![];
        let diff_json = serde_json::to_string(&diff_rows).expect("serialize");
        let input = format!(
            r#"{{"id":3,"method":"report","params":{{"diff":{},"fail_on":"bind_drift"}}}}"#,
            serde_json::Value::String(diff_json)
        );
        let output = run_serve(&input);
        let resp: serde_json::Value = serde_json::from_str(output.trim()).expect("valid json");
        assert!(resp["result"]["summary"]["should_fail"].as_bool() == Some(false));
    }

    #[test]
    fn test_serve_parse_error() {
        let input = "not-valid-json";
        let output = run_serve(input);
        let resp: serde_json::Value = serde_json::from_str(output.trim()).expect("valid json");
        assert!(resp["error"]["message"].as_str().unwrap().contains("parse error"));
    }
}
