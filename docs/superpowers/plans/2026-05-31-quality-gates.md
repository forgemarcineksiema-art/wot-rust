# Quality Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn engineering expectations into executable workspace gates and documentation.

**Architecture:** Add a small `quality` crate for architecture rules, keep protocol snapshots in `net`, replay regression fixtures in `sim`, and benchmark targets beside the crate they measure. Verification is centralized in a PowerShell script and mirrored in CI.

**Tech Stack:** Cargo workspace, rustfmt, clippy, Criterion, serde JSON replay fixtures, bincode protocol snapshots, GitHub Actions.

---

### Task 1: Executable Rules

**Files:**
- Create: `crates/quality`
- Create: `scripts/verify.ps1`
- Create: `.github/workflows/ci.yml`

- [x] Add a `quality` crate with architecture tests.
- [x] Enforce max 220 Rust lines per file through tests.
- [x] Add one command for rustfmt, clippy, tests, check, and benchmark compilation.
- [x] Add CI that runs the same verification script.

### Task 2: Protocol Snapshot Tests

**Files:**
- Create: `crates/net/tests/protocol_snapshots.rs`
- Create: `crates/net/tests/snapshots/input_command_v1.hex`
- Modify: `crates/net/src/lib.rs`

- [x] Add a binary protocol snapshot fixture.
- [x] Add protocol version constant.
- [x] Update the fixture after the first red snapshot run.

### Task 3: Replay Regression Tests

**Files:**
- Create: `crates/sim/tests/replay_regression.rs`
- Create: `crates/sim/tests/replays/drive_forward_v1.json`
- Modify: `crates/sim/src/lib.rs`

- [x] Add a replay fixture for fixed-tick drive/turret behavior.
- [x] Add replay data types and runner.

### Task 4: Benchmarks And Docs

**Files:**
- Create: `crates/sim/benches/fixed_tick.rs`
- Create: `crates/net/benches/protocol_codec.rs`
- Create: `docs/engineering-rules.md`
- Create: `docs/testing-and-regression.md`
- Modify: `docs/architecture.md`

- [x] Add Criterion benchmark targets.
- [x] Document hard engineering rules.
- [x] Document protocol snapshot, replay, and benchmark workflows.
- [x] Update architecture docs with module boundaries and executable quality gates.
