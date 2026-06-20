# mqo-replay

Replay logged agent decisions through the current `mqo-agent` and flag where its behavior moved.

An agent's behavior is invisible between releases. The planner, the binders, the pillar tools — any of them can quietly change what the agent does with a given question, and the model contract that `mqo-semantic-regression` watches stays green the whole time. The grain didn't change; the PII tags didn't change; the agent just routed the question differently. `mqo-replay` makes that drift visible by treating every past decision as a regression test: it re-runs the questions from a decision log through the current agent and diffs the new answer against what was recorded.

This is a behavioral gate, not an accuracy benchmark — that's `mqo-bench`. It compares the agent against its *own* past behavior, so a flagged drift is a prompt to review, not a verdict that the new answer is wrong.

## Install

```bash
cargo install --path .
```

Or build and copy the binary:

```bash
cargo build --release
cp target/release/mqo-replay ~/.local/bin/
```

## How it works

The unit of work is a `DecisionRecord` — one logged decision from `mqo-decision-log`: the question, the plan, the access verdict, the pillars that fired, the outcome (`answered` / `clarify` / `blocked`), and the optional bound measure, grain, and numeric answer. `mqo-replay` mirrors that schema rather than depending on the `mqo-decision-log` crate, so it stays an independent observer and can run against any version of the agent.

Three steps, each its own subcommand:

1. **`run`** feeds each logged question back through the current agent and records the fresh decision.
2. **`diff`** matches fresh records to their baselines and classifies what moved.
3. **`report`** summarizes the classes and sets an exit code you can gate CI on.

`diff` assigns each question exactly one class, by priority — the first thing that changed wins:

| Class | Fires when |
|----|----|
| `outcome_drift` | the outcome changed (`answered` → `clarify`, etc.) |
| `bind_drift` | the bound measure or grain changed |
| `plan_drift` | the set of pillars that fired changed (order ignored) |
| `value_drift` | the numeric answer moved beyond tolerance |
| `unchanged` | none of the above |

## Quickstart

Given a decision log `decisions.jsonl` (one JSON `DecisionRecord` per line):

```bash
# 1. Replay the logged questions through the current agent.
mqo-replay run --log decisions.jsonl --agent mqo-agent -o fresh.jsonl

# 2. Classify what moved.
mqo-replay diff --baseline decisions.jsonl --replay fresh.jsonl -o diff.json

# 3. Summarize and gate.
mqo-replay report --diff diff.json --fail-on bind_drift,value_drift
```

`report` prints a count per class and the first drifted questions, then exits non-zero if any class named in `--fail-on` is present:

```
mqo-replay report: 42 questions
─────────────────────────────────
  bind_drift      1
  plan_drift      3
  unchanged       38

Worst offenders:
  [bind_drift   ] what was Q3 revenue by region?

FAIL: drift classes present: bind_drift
```

The pattern that earns its place in CI: gate on `bind_drift` and `value_drift` — a different measure or a different number is a real regression — while tolerating `plan_drift`, where the agent reached the same answer by a different route.

## Subcommands

### `run`

```bash
mqo-replay run --log <LOG> [--agent <CMD>] [--since <ISO-8601>] [-o <OUTPUT>]
```

Replays each logged question. `--agent` defaults to `mqo-agent`; pass any command to test a candidate build. `--since` filters to records at or after an ISO-8601 timestamp. Output goes to `--output` or stdout.

### `diff`

```bash
mqo-replay diff --baseline <BASELINE> --replay <REPLAY> [--value-tol <T>] [--relative] [-o <OUTPUT>]
```

Matches fresh records to baselines by question text and classifies each. Numeric comparison uses the same convention as `mqo-engine-parity`: absolute tolerance by default (`--value-tol`, 1e-6), relative with `--relative`. A value present on one side and absent on the other counts as `value_drift`.

### `report`

```bash
mqo-replay report --diff <DIFF> [--fail-on <CLASSES>] [--format text|json]
```

Summarizes a diff and exits non-zero when any comma-separated class in `--fail-on` is present. `--format json` emits the structured summary instead of the text rendering.

### `serve`

```bash
mqo-replay serve
```

Exposes `run`, `diff`, and `report` as newline-delimited JSON-RPC over stdin/stdout. Each request is `{"id": ..., "method": "run"|"diff"|"report", "params": {...}}`; each response carries a `result` or an `error`. The `params` mirror the CLI flags, with log contents passed inline as strings.

## Agent subprocess protocol

The agent runs as `sh -c <cmd>`. The question is supplied two ways — as the `MQO_REPLAY_QUESTION` environment variable and written to the agent's stdin — and the agent must emit one JSON `DecisionRecord` on stdout. A trivial stand-in for testing is a shell command that echoes a fixed record, which is exactly how the test suite drives `run` without a cluster.

## Where it fits

Part of the mqo regression family, each watching a different surface:

- **`mqo-replay`** — agent *behavior* against its own decision history (this repo).
- **`mqo-semantic-regression`** — the model *contract*: grain, PII tags.
- **`mqo-bench`** — golden *accuracy*.
- **`mqo-engine-parity`** — numeric agreement across engines; `mqo-replay` borrows its tolerance convention.

## Status

Works against bundled fixtures, offline, with eight acceptance tests covering each subcommand and the tolerance boundary (`cargo test`). It has not yet been run against a live `mqo-agent` over a production decision log; the subprocess protocol is the contract that path will use.

## License

MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
