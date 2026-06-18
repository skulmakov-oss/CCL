# Command Evidence Capture Seed Gate

Status: Draft  
Gate type: implementation seed  
Expected PR title: `feat: add command evidence capture seed`  
Primary goal: create CCL-owned command execution evidence

---

## 1. Purpose

This gate defines the first real root-of-trust implementation step for CCL after the CLI core seed.

The CLI core can load task contracts, run preflight checks, and represent evidence/verdict types. The next step is to make CCL produce its own execution evidence.

Principle:

```text
Capture first.
Interpret later.
Admit only from captured evidence.
```

The goal of this gate is not to understand tool output yet. The goal is to prove that CCL can launch a command, bound its execution, capture stdout/stderr, record environment context, compute hashes, and persist a reproducible evidence artifact.

---

## 2. Non-goals

This gate must not implement:

- `ccl gate run`;
- full Admission Guard;
- scope/diff policy engine;
- Diagnostic Extractors;
- Failure Capsule;
- LLM hints;
- Broker/MCP integration;
- sandbox mode;
- UI/Tauri;
- GitHub Actions;
- license changes.

This is a low-level sensor gate only.

---

## 3. Expected CLI

Add a new CLI command:

```powershell
cargo run -p ccl-cli -- capture --id cargo-version --repo . -- cargo --version
```

Optional timeout form:

```powershell
cargo run -p ccl-cli -- capture --id cargo-test --repo . --wall-timeout 300 -- cargo test
```

The command after `--` must be treated as argv:

```text
program = cargo
args = ["test"]
```

Shell execution must not be the default.

---

## 4. Artifact Layout

The command must create a run directory:

```text
.ccl/runs/<run-id>/
  run.json
  evidence-manifest.json
  commands/
    001-<command-id>/
      command.json
      env.json
      stdout.txt
      stderr.txt
      result.json
```

The generated `.ccl/runs/**` content must remain local runtime output and must not be committed.

---

## 5. Required Captured Fields

`command.json` should record:

- command id;
- program;
- args;
- repo path;
- cwd;
- wall timeout;
- environment policy;
- created timestamp.

`env.json` should record a snapshot of relevant environment variables for MVP.

`result.json` should record:

- command id;
- status: `PASS` / `FAIL`;
- failure class if applicable;
- exit code;
- timed out: true / false;
- runtime milliseconds;
- stdout path;
- stderr path;
- stdout SHA-256;
- stderr SHA-256;
- env SHA-256;
- command artifact path.

`evidence-manifest.json` should include:

- run id;
- repo path;
- command evidence list;
- aggregate status;
- artifact paths.

---

## 6. Timeout Policy

Every captured command must be bounded.

Minimum MVP field:

```json
{
  "wall_timeout_seconds": 300
}
```

If timeout is exceeded:

```text
status = FAIL
failure_class = timeout_exceeded
exit_code = null
```

Partial stdout/stderr must still be saved and hashed.

Target behavior for future hardening:

- terminate whole process tree;
- Unix: process group + SIGTERM/SIGKILL;
- Windows: Job Object or process tree fallback.

The MVP may use a simpler cross-platform approach, but the code and docs must not pretend it is full sandbox-level process isolation.

---

## 7. Environment Evidence

Environment is part of the evidence surface.

MVP behavior:

```text
capture environment snapshot and hash it
```

The implementation should at minimum make environment capture visible in artifacts. A future gate may add strict environment allowlists.

Important Rust-related variables to consider in docs/tests:

```text
RUSTFLAGS
RUSTDOCFLAGS
CARGO_ENCODED_RUSTFLAGS
RUST_TEST_THREADS
RUST_BACKTRACE
CARGO_TARGET_DIR
```

---

## 8. Expected Code Scope

Allowed files:

```text
crates/ccl-core/src/evidence.rs
crates/ccl-core/src/capture.rs
crates/ccl-core/src/lib.rs
crates/ccl-cli/src/main.rs
crates/ccl-core/Cargo.toml
crates/ccl-cli/Cargo.toml
Cargo.lock
README.md
docs/roadmap.md
ledger/project-ledger.md
```

Forbidden files:

```text
.github/**
LICENSE
UI/Tauri files
CCL_DNA.md, unless PR #4 is merged and the maintainer explicitly requests a DNA update
docs/architecture.md
docs/task-contract.md
docs/agent-report-format.md
docs/project-ledger.md
examples/**
```

---

## 9. Dependency Policy

Keep dependencies minimal.

Possible dependency needs:

- SHA-256 hashing;
- timestamp generation;
- JSON serialization already present;
- temporary directory support for tests if needed.

No dependency should be added for UI, async orchestration, LLM calls, GitHub API, or sandboxing in this gate.

---

## 10. Required Tests

Add tests for:

1. successful command capture, using a small deterministic command;
2. non-zero exit command capture;
3. stdout/stderr files are created;
4. hashes are present and stable for captured files;
5. evidence manifest is created;
6. timeout produces `FAIL` with `timeout_exceeded` if feasible in a stable test;
7. env snapshot artifact exists.

Tests must avoid relying on network access.

---

## 11. Required Validation

After implementation, run locally:

```powershell
git status --short --branch
git diff --check
cargo fmt --check
cargo test
cargo run -p ccl-cli -- --version
cargo run -p ccl-cli -- contract check examples/semantic-task-contract.json
cargo run -p ccl-cli -- preflight --repo .
cargo run -p ccl-cli -- capture --id cargo-version --repo . -- cargo --version
cargo clippy --all-targets --all-features -- -D warnings
```

Do not use GitHub CI as validation evidence.

---

## 12. Ledger Requirement

Update `ledger/project-ledger.md` with a new entry for this gate.

Status should be:

```text
PASS WITH WARNINGS
```

unless a real CCL Admission Guard exists by the time this gate is completed.

Expected warning:

```text
CCL local Admission Guard is not implemented yet.
```

The ledger must record:

- branch;
- PR number;
- changed files;
- validation results;
- command capture proof command;
- artifact shape;
- next recommended gate.

---

## 13. Expected Final Report

The implementation agent must report:

```text
Created / edited files
Commands run
Validation results
Capture artifact path
Example result.json summary
Ledger status
PR number
GitHub CI used as evidence: NO
```

No `PASS` may be claimed without local validation output.

---

## 14. Next Gate After This

After Command Evidence Capture Seed, the next likely gate is:

```text
Evidence Manifest + Contract-bound Validation Runner
```

Only after that should CCL grow toward:

```text
ccl gate run
scope/diff policy checks
Diagnostic Extractors
Failure Capsule
Retry Contract / Circuit Breaker
```
