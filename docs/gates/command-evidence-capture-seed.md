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

This gate must treat the existing Local Admission Guard as a real validation backend. CCL must not replace it in this seed. CCL must prove that it can capture the Local Admission Guard run as evidence.

---

## 2. Non-goals

This gate must not implement:

- `ccl gate run`;
- full CCL Admission Layer;
- replacement or reimplementation of the existing Local Admission Guard;
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

## 3. Existing Local Admission Guard Relationship

The project already has a functional Local Admission Guard that acts as a fast local CI / validator backend.

CCL must treat that existing guard as an external validation command in this seed.

Correct relationship:

```text
Local Admission Guard checks.
CCL Capture proves the check happened.
CCL Evidence Manifest preserves the proof.
CCL Verdict later decides admission.
```

This means:

- CCL must not claim to replace the Local Admission Guard in this gate;
- CCL must not trust a handwritten agent report that says the guard passed;
- CCL must capture the Local Admission Guard process execution as evidence;
- captured evidence must include command, cwd, env snapshot/hash, stdout/stderr, exit code, timeout/output-limit status, and artifact hashes;
- GitHub CI remains secondary metadata, not final evidence.

The first production-like capture target for this gate should be the existing Local Admission Guard, not only a trivial command such as `cargo --version`.

---

## 4. Expected CLI

Add a new CLI command:

```powershell
cargo run -p ccl-cli -- capture --id cargo-version --repo . -- cargo --version
```

Optional timeout form:

```powershell
cargo run -p ccl-cli -- capture --id cargo-test --repo . --wall-timeout 300 -- cargo test
```

Production-like Local Admission Guard capture form:

```powershell
cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- <local-admission-guard-command>
```

The exact Local Admission Guard command is repository-specific and must be documented by the implementation agent in the final report.

The command after `--` must be treated as argv:

```text
program = cargo
args = ["test"]
```

Shell execution must not be the default. If a shell or script runner is needed, it must appear explicitly as the program in argv and be justified in the final report.

---

## 5. Artifact Layout

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

## 6. Required Captured Fields

`command.json` should record:

- command id;
- program;
- args;
- repo path;
- cwd;
- wall timeout;
- output byte limits;
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
- stdout bytes;
- stderr bytes;
- combined output bytes;
- stdout complete: true / false;
- stderr complete: true / false;
- stdout truncated: true / false;
- stderr truncated: true / false;
- output limit exceeded: true / false;
- max stdout bytes;
- max stderr bytes;
- max combined output bytes;
- hash scope for each output stream;
- command artifact path.

`evidence-manifest.json` should include:

- run id;
- repo path;
- command evidence list;
- aggregate status;
- artifact paths;
- whether the Local Admission Guard was captured;
- path to the Local Admission Guard command result if captured.

---

## 7. Timeout Policy

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

After timeout termination, stdout/stderr reader threads must complete deterministically. They should drain any final bytes until EOF after the child process is killed or terminated, then flush files and finalize hashes.

The implementation must not wait forever for stream readers after timeout. Reader joins and post-kill stream draining must be bounded by a small internal deadline. If stream draining or reader shutdown cannot complete, the command result must remain `FAIL` and record an explicit failure class such as `stream_drain_failed` or `io_error`.

Target behavior for future hardening:

- terminate whole process tree;
- Unix: process group + SIGTERM/SIGKILL;
- Windows: Job Object or process tree fallback.

The MVP may use a simpler cross-platform approach, but the code and docs must not pretend it is full sandbox-level process isolation.

---

## 8. Environment Evidence

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

## 9. Output Streaming and Size Limits

CCL must not buffer full stdout/stderr in memory.

Captured output must be streamed to disk in chunks while updating rolling SHA-256 hashes and byte counters.

Required model:

```text
child stdout pipe -> chunk reader -> stdout.txt -> rolling sha256
child stderr pipe -> chunk reader -> stderr.txt -> rolling sha256
```

The implementation must read stdout and stderr concurrently. Reading one stream while ignoring the other can deadlock the child process when the ignored pipe fills.

Minimum MVP behavior:

- stream stdout to `stdout.txt`;
- stream stderr to `stderr.txt`;
- update stdout hash per chunk;
- update stderr hash per chunk;
- track stdout bytes;
- track stderr bytes;
- track combined output bytes;
- enforce output byte limits;
- keep memory usage bounded relative to output size.

Recommended MVP limits:

```json
{
  "max_stdout_bytes": 10485760,
  "max_stderr_bytes": 10485760,
  "max_combined_output_bytes": 20971520,
  "on_output_limit": "fail_and_terminate"
}
```

If any output limit is exceeded:

```text
status = FAIL
failure_class = output_limit_exceeded
exit_code = null
```

Partial logs must still be saved and hashed.

If stdout/stderr is truncated, hashes must be computed over the bytes actually saved to disk, not over missing or imagined full output.

Expected result fields:

```json
{
  "stdout_bytes": 10485760,
  "stderr_bytes": 18342,
  "combined_output_bytes": 10504102,
  "stdout_complete": false,
  "stderr_complete": true,
  "stdout_truncated": true,
  "stderr_truncated": false,
  "output_limit_exceeded": true,
  "max_stdout_bytes": 10485760,
  "max_stderr_bytes": 10485760,
  "max_combined_output_bytes": 20971520,
  "stdout_hash_scope": "saved_bytes_only",
  "stderr_hash_scope": "saved_bytes_only"
}
```

For this seed, `fail_and_terminate` is the preferred behavior. A future gate may add `truncate_and_continue`, but the MVP should be conservative.

Principle:

```text
Capture must be streaming, bounded, hashed, and backpressure-safe.
```

---

## 10. Expected Code Scope

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

## 11. Dependency Policy

Keep dependencies minimal and explicit.

Allowed / preferred dependency choices for this gate:

- `sha2 = "0.10"` for SHA-256 hashing;
- `hex = "0.4"` for digest encoding;
- existing `serde` / `serde_json` for artifact serialization;
- `std::time::SystemTime` for MVP timestamps;
- `tempfile` as a dev-dependency only, if needed for stable tests.

Avoid adding `chrono` in this seed unless there is a clear documented need. Human-friendly timestamp formatting can be added later.

Do not add:

- Tokio or another async runtime;
- command wrapper frameworks;
- UI dependencies;
- LLM or API client dependencies;
- GitHub API dependencies;
- sandbox/container orchestration dependencies;
- broad logging/tracing frameworks unless explicitly justified.

No dependency should be added for UI, async orchestration, LLM calls, GitHub API, or sandboxing in this gate.

---

## 12. Required Tests

Add tests for:

1. successful command capture, using a small deterministic command;
2. non-zero exit command capture;
3. stdout/stderr files are created;
4. hashes are present and stable for captured files;
5. evidence manifest is created;
6. timeout produces `FAIL` with `timeout_exceeded` if feasible in a stable test;
7. env snapshot artifact exists;
8. stdout/stderr are streamed to disk rather than buffered as a full in-memory result;
9. output byte counters are recorded;
10. output limits produce `FAIL` with `output_limit_exceeded` if feasible in a stable test;
11. truncated output is explicitly marked incomplete and hashed as saved bytes only;
12. stdout/stderr are read concurrently enough to avoid pipe deadlock.

Backpressure/deadlock test requirement:

- add a deterministic test helper that writes more than a typical OS pipe buffer to stdout and stderr;
- the helper should produce enough output to expose sequential-reader deadlocks, for example `1 MiB` to stdout and `1 MiB` to stderr;
- the capture must complete under a bounded timeout when both streams are pumped concurrently;
- the test should avoid Python/Bash dependency by default. Prefer a Rust test helper, current test binary mode, or another cross-platform Rust-controlled helper;
- if an external interpreter is used, the test must be gated and must not be the only proof of backpressure safety.

Tests must avoid relying on network access.

---

## 13. Required Validation

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
cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- <local-admission-guard-command>
cargo clippy --all-targets --all-features -- -D warnings
```

The implementation agent must replace `<local-admission-guard-command>` with the real repository command and report the exact command used.

The Local Admission Guard capture is required as the production-like proof command for this gate. `cargo --version` is only a smoke test for trivial command capture.

Do not use GitHub CI as validation evidence.

---

## 14. Ledger Requirement

Update `ledger/project-ledger.md` with a new entry for this gate.

Status should be:

```text
PASS WITH WARNINGS
```

unless the implementation also includes a complete CCL-owned admission layer that can compute the final project verdict from captured evidence.

Expected warning for this seed:

```text
Existing Local Admission Guard is available and must be captured as validation evidence, but the full CCL admission layer is not implemented yet.
```

The ledger must record:

- branch;
- PR number;
- changed files;
- validation results;
- command capture proof command;
- Local Admission Guard capture command;
- Local Admission Guard capture result;
- artifact shape;
- streaming stdout/stderr behavior;
- output byte limits;
- timeout stream-drain behavior;
- backpressure/deadlock test result;
- next recommended gate.

Ledger must explicitly state:

```text
Local Admission Guard executed through CCL capture: YES / NO
GitHub CI used as evidence: NO
```

---

## 15. Expected Final Report

The implementation agent must report:

```text
Created / edited files
Commands run
Validation results
Capture artifact path
Example result.json summary
Local Admission Guard command captured: YES / NO
Local Admission Guard exact command: <command>
Local Admission Guard capture result: PASS / FAIL / NOT RUN, with reason
Streaming stdout/stderr: YES / NO
Output byte limits enforced: YES / NO
Timeout stream drain bounded: YES / NO
Backpressure/deadlock test: PASS / FAIL / NOT RUN, with reason
Ledger status
PR number
GitHub CI used as evidence: NO
```

No `PASS` may be claimed without local validation output.

---

## 16. Next Gate After This

After Command Evidence Capture Seed, the next likely gate is:

```text
Evidence Manifest + Contract-bound Validation Runner
```

That next gate should use the existing Local Admission Guard as one of the primary validation backends while CCL owns orchestration, evidence persistence, and verdict inputs.

Only after that should CCL grow toward:

```text
ccl gate run
scope/diff policy checks
Diagnostic Extractors
Failure Capsule
Retry Contract / Circuit Breaker
```
