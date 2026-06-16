//! `mqo-replay` — behavioral regression replay of mqo-agent against its own decision history.
//!
//! Consumes a `mqo-decision-log` JSONL corpus and re-runs each logged question through
//! the current agent (or a mock), then diffs the results into drift categories.

pub mod diff;
pub mod record;
pub mod replay;
pub mod report;
pub mod serve;

pub use diff::{DiffClass, DiffRow, DiffConfig};
pub use record::DecisionRecord;
pub use replay::{ReplayConfig, ReplayRecord};
pub use report::{ReportSummary, ReportConfig};
