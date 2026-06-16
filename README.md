# mqo-replay

Behavioral regression replay of mqo-agent against its own decision history.

## Overview

`mqo-replay` re-runs NL questions from a `mqo-decision-log` through the current `mqo-agent` and diffs the new plan/bind/outcome against what was logged, surfacing behavioral drift before it reaches a user.

This is a behavioral regression gate, not an accuracy benchmark (that's `mqo-bench`). It compares the agent against its *own past behavior* — a drift is a flag for review, not a verdict on correctness.

## Installation

```bash
cargo install --path .
# or copy from release:
cp target/release/mqo-replay ~/.local/bin/
```

## Usage

### `run` — replay logged questions through the current agent

```bash
mqo-replay run --log decisions.jsonl --agent mqo-agent [--since 2026-01-01T00:00:00Z] [-o fresh.jsonl]
```

Extracts each logged question, invokes the agent subprocess on it, and records the fresh `DecisionRecord` JSON.

### `diff` — classify drift between baseline and fresh logs

```bash
mqo-replay diff --baseline decisions.jsonl --replay fresh.jsonl [--value-tol 0.01] [--relative] [-o diff.json]
```

Drift categories (in priority order):
- `outcome_drift` — outcome field changed (answered/clarify/blocked)
- `bind_drift` — bound measure or grain changed
- `plan_drift` — pillars fired set changed
- `value_drift` — numeric answer moved beyond `--value-tol`
- `unchanged` — no change detected

### `report` — summarize drift and gate on classes

```bash
mqo-replay report --diff diff.json [--fail-on bind_drift,value_drift] [--format json|text]
```

Exits non-zero when any class in `--fail-on` is present. Use in CI to block releases on `bind_drift` or `value_drift` while tolerating benign `plan_drift`.

### `serve` — expose as MCP tool server

```bash
mqo-replay serve
```

Reads JSON-RPC requests from stdin, writes responses to stdout. Methods: `run`, `diff`, `report`.

## Tolerance convention

Numeric comparison uses the same tolerance convention as `mqo-engine-parity`:
- Absolute tolerance by default (`--value-tol 0.000001`)
- Relative tolerance with `--relative` flag

## Agent subprocess protocol

The agent is invoked via `sh -c <cmd>`. The question is passed as the `MQO_REPLAY_QUESTION` environment variable and also written to the agent's stdin. The agent must emit a JSON `DecisionRecord` on stdout.

## Acceptance criteria

| AC | Description |
|----|-------------|
| AC1 | `run` produces one fresh record per logged question |
| AC2 | `diff` against unchanged agent → all `unchanged` |
| AC3 | `diff` against drifted agent → labels `plan_drift`, `bind_drift`, `outcome_drift` |
| AC4 | `value_drift` boundary tested at tolerance edge |
| AC5 | `report --fail-on` exits non-zero on matching classes, zero otherwise |
| AC6 | Numeric tolerance matches `mqo-engine-parity` convention |
| AC7 | `diff`/`report` output is deterministic for fixed inputs |
| AC8 | `serve` answers tool calls; all tests cluster-free |

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
