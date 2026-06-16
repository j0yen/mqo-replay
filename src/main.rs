//! `mqo-replay` CLI — behavioral regression replay of mqo-agent.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, BufReader};

use mqo_replay::diff::{DiffConfig, diff_logs};
use mqo_replay::record::parse_log;
use mqo_replay::replay::{ReplayConfig, run_replay};
use mqo_replay::report::{ReportConfig, generate_report, render_report};
use mqo_replay::serve::serve_loop;

#[derive(Parser)]
#[command(
    name = "mqo-replay",
    about = "Behavioral regression replay of mqo-agent against its own decision history",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Re-run logged questions through the current agent and record fresh answers
    Run {
        /// Path to the decision log JSONL file
        #[arg(long, short = 'l')]
        log: String,
        /// Agent command to invoke (default: mqo-agent)
        #[arg(long, short = 'a', default_value = "mqo-agent")]
        agent: String,
        /// Only replay records at or after this ISO-8601 timestamp
        #[arg(long)]
        since: Option<String>,
        /// Output file for fresh records (default: stdout)
        #[arg(long, short = 'o')]
        output: Option<String>,
    },
    /// Compare a baseline decision log against a fresh replay log
    Diff {
        /// Baseline decisions JSONL file
        #[arg(long)]
        baseline: String,
        /// Fresh replay JSONL file
        #[arg(long)]
        replay: String,
        /// Absolute tolerance for value_drift (default: 1e-6)
        #[arg(long, default_value = "0.000001")]
        value_tol: f64,
        /// Use relative tolerance instead of absolute
        #[arg(long)]
        relative: bool,
        /// Output file for diff JSON (default: stdout)
        #[arg(long, short = 'o')]
        output: Option<String>,
    },
    /// Render a summary of diff output and exit non-zero on specified drift classes
    Report {
        /// Diff JSON file (output of `diff` subcommand)
        #[arg(long, short = 'd')]
        diff: String,
        /// Comma-separated drift classes that trigger non-zero exit
        /// (e.g. bind_drift,value_drift)
        #[arg(long, default_value = "")]
        fail_on: String,
        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Serve run/diff/report as JSON-RPC tools on stdin/stdout
    Serve,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            log,
            agent,
            since,
            output,
        } => {
            let content = fs::read_to_string(&log)
                .with_context(|| format!("reading log file: {}", log))?;
            let baseline = parse_log(&content)?;

            let config = ReplayConfig { agent_cmd: agent, since };
            let replays = run_replay(&baseline, &config)?;

            let json = serde_json::to_string_pretty(&replays)?;
            if let Some(out_path) = output {
                fs::write(&out_path, json)
                    .with_context(|| format!("writing output: {}", out_path))?;
            } else {
                println!("{}", json);
            }
        }

        Commands::Diff {
            baseline,
            replay,
            value_tol,
            relative,
            output,
        } => {
            let baseline_content = fs::read_to_string(&baseline)
                .with_context(|| format!("reading baseline: {}", baseline))?;
            let replay_content = fs::read_to_string(&replay)
                .with_context(|| format!("reading replay: {}", replay))?;

            let baseline_recs = parse_log(&baseline_content)?;
            let replay_recs = parse_log(&replay_content)?;

            let config = DiffConfig { value_tol, relative };
            let rows = diff_logs(&baseline_recs, &replay_recs, &config);

            let json = serde_json::to_string_pretty(&rows)?;
            if let Some(out_path) = output {
                fs::write(&out_path, json)
                    .with_context(|| format!("writing output: {}", out_path))?;
            } else {
                println!("{}", json);
            }
        }

        Commands::Report {
            diff,
            fail_on,
            format,
        } => {
            let diff_content = fs::read_to_string(&diff)
                .with_context(|| format!("reading diff file: {}", diff))?;
            let rows: Vec<mqo_replay::diff::DiffRow> = serde_json::from_str(&diff_content)
                .context("parsing diff JSON")?;

            let fail_on_classes = ReportConfig::parse_fail_on(&fail_on)?;
            let config = ReportConfig { fail_on: fail_on_classes };
            let summary = generate_report(&rows, &config);

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
                _ => {
                    print!("{}", render_report(&summary));
                }
            }

            if summary.should_fail {
                std::process::exit(1);
            }
        }

        Commands::Serve => {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            serve_loop(reader, stdout.lock())?;
        }
    }

    Ok(())
}
