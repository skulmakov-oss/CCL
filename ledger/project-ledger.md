# CCL Project Ledger

## 2026-06-17 — Phase 1 CLI Core Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: CCL Phase 1
- Task type: source_pr
- Branch: phase1/cli-core-seed
- PR: https://github.com/skulmakov-oss/CCL/pull/2
- Merge commit: 449b5d86740122f188bfc64d46058ff839fb0605
- Final main HEAD after merge: 449b5d86740122f188bfc64d46058ff839fb0605

### Basis

- README.md
- CONTRIBUTING.md
- docs/architecture.md
- docs/task-contract.md
- docs/agent-report-format.md
- docs/project-ledger.md
- docs/roadmap.md
- examples/semantic-task-contract.json

### Changed Files

Created:
- Cargo.toml
- crates/ccl-core/Cargo.toml
- crates/ccl-core/src/evidence.rs
- crates/ccl-core/src/lib.rs
- crates/ccl-core/src/preflight.rs
- crates/ccl-core/src/task_contract.rs
- crates/ccl-core/src/verdict.rs
- crates/ccl-cli/Cargo.toml
- crates/ccl-cli/src/main.rs
- ledger/project-ledger.md

Edited:
- README.md
- docs/roadmap.md

Deleted:
- none

### Validation

- `cargo fmt --check`: PASS
- `cargo test`: PASS
- `cargo run -p ccl-cli -- contract check examples/semantic-task-contract.json`: PASS
- `cargo run -p ccl-cli -- preflight --repo .`: PASS
- GitHub CI used as evidence: NO

### Warnings

- CCL local Admission Guard is not implemented yet.
- This gate seeds the deterministic CLI core only.

### Boundary Conclusion

- semantic authority changed: NO
- CCL verdict model seeded: YES
- agent governance boundary preserved: YES
- dependency surface changed: YES, minimal Rust CLI dependencies only

### Next Gate

- recommended next gate: command evidence capture
- reason: after contract loading and preflight, CCL needs deterministic command output capture
- expected files: crates/ccl-core/src/evidence.rs, crates/ccl-cli/src/main.rs, ledger/project-ledger.md
- forbidden files: .github/**, LICENSE, UI/Tauri files

## 2026-06-20 — Command Evidence Capture Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: CCL Phase 1
- Task type: capture seed
- Branch: feat/command-evidence-capture-seed
- PR: not created
- Merge commit: not applicable
- Final main HEAD after merge: not applicable

### Basis

- README.md
- CCL_DNA.md
- docs/gates/command-evidence-capture-seed.md
- docs/roadmap.md
- ledger/project-ledger.md
- crates/ccl-core/src/evidence.rs
- crates/ccl-core/src/verdict.rs
- crates/ccl-core/src/lib.rs
- crates/ccl-cli/src/main.rs
- crates/ccl-core/Cargo.toml
- crates/ccl-cli/Cargo.toml

### Changed Files

Created:
- crates/ccl-core/src/capture.rs

Edited:
- Cargo.lock
- crates/ccl-cli/src/main.rs
- crates/ccl-core/Cargo.toml
- crates/ccl-core/src/evidence.rs
- crates/ccl-core/src/lib.rs
- ledger/project-ledger.md

Deleted:
- none

### Validation

- `git status --short --branch`: PASS
- `git diff --check`: PASS
- `cargo fmt --check`: PASS
- `cargo test`: PASS
- `cargo run -p ccl-cli -- --version`: PASS
- `cargo run -p ccl-cli -- contract check examples/semantic-task-contract.json`: PASS
- `cargo run -p ccl-cli -- preflight --repo .`: PASS
- `cargo run -p ccl-cli -- capture --id cargo-version --repo . -- cargo --version`: PASS
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- GitHub CI used as evidence: NO

### Capture Proof

- cargo-version capture: PASS
- artifact path: `C:\Users\said3\Desktop\CCL\.ccl\runs\1781963867501-18332\commands\001-cargo-version\result.json`
- capture artifact shape:
  - `.ccl/runs/<run-id>/run.json`
  - `.ccl/runs/<run-id>/evidence-manifest.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/command.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/env.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/stdout.txt`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/stderr.txt`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/result.json`
- Local Admission Guard command: not determinable from repository state
- Local Admission Guard executed through CCL capture: NO
- Local Admission Guard capture result: NOT RUN, because the exact repository command could not be identified from the checked-in files
- streaming stdout/stderr: YES
- output byte limits enforced: YES
- timeout stream drain bounded: YES
- backpressure/deadlock test result: PASS

### Warnings

- Existing Local Admission Guard is available in project doctrine, but the exact repository command was not present in checked-in files, so a production-like Local Admission Guard capture could not be run without inventing the command.
- The full CCL admission layer is not implemented yet.
- GitHub CI was not used as evidence.

### Boundary Conclusion

- ccl capture command added: YES
- argv execution, no shell by default: YES
- stdout/stderr streaming: YES
- concurrent pipe reading: YES
- wall-timeout: YES
- output byte limits: YES
- env snapshot: YES
- SHA-256 hashes: YES
- result.json: YES
- evidence-manifest.json: YES

### Next Gate

- recommended next gate: Evidence Manifest + Contract-bound Validation Runner
- reason: command capture is now available; next step is to bind validation to manifest-backed admission logic
