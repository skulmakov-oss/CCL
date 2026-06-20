# CCL Project Ledger

## 2026-06-20 — Evidence Manifest + Contract-bound Validation Runner

Status: PASS WITH WARNINGS

### Scope

- Workstream: CCL Phase 1
- Task type: validation runner
- Branch: feat/contract-bound-validation-runner
- PR: #7 — https://github.com/skulmakov-oss/CCL/pull/7
- Base main HEAD: 84a81f970b581dd3e4e3fa3cf44c4e46596f9e12

### Basis

- README.md
- docs/roadmap.md
- ledger/project-ledger.md
- examples/semantic-task-contract.json
- examples/ccl-validation-task-contract.json
- crates/ccl-core/src/task_contract.rs
- crates/ccl-core/src/validation_runner.rs
- crates/ccl-core/src/evidence.rs
- crates/ccl-core/src/lib.rs
- crates/ccl-cli/src/main.rs
- ci/admission.ps1
- ci/admission.sh
- ci/rust_gate.sh

### Changed Files

Created:
- crates/ccl-core/src/validation_runner.rs
- examples/ccl-validation-task-contract.json

Edited:
- README.md
- crates/ccl-cli/src/main.rs
- crates/ccl-core/src/lib.rs
- crates/ccl-core/src/task_contract.rs
- docs/roadmap.md
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
- `cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- powershell.exe -File .\ci\admission.ps1 --full`: PASS
- `cargo run -p ccl-cli -- validate run --contract examples/ccl-validation-task-contract.json --repo .`: PASS
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- GitHub CI used as evidence: NO

### Validation Contract Used

- `examples/ccl-validation-task-contract.json`

### Aggregate Manifest

- `.ccl/runs/validation-1781977468189-18536/validation-run-manifest.json`

### Command Capture Result Paths

- `.ccl/runs/1781977520287-2348/commands/001-cargo-version/result.json`
- `.ccl/runs/1781977525432-27656/commands/001-local-admission-guard/result.json`
- `.ccl/runs/1781977473001-18536/commands/001-local-admission-guard/result.json`

### Validation Results

- ccl capture command added: YES
- contract validation commands parsed: YES
- commands executed through ccl capture: YES
- aggregate validation manifest written: YES
- contract SHA-256 recorded: YES
- Local Admission Guard runnable through validation contract: YES
- GitHub CI used as evidence: NO
- streaming stdout/stderr: YES
- output byte limits enforced: YES
- timeout stream drain bounded: YES
- backpressure/deadlock test result: PASS

### Warnings

- Full CCL admission layer is still not implemented; this PR only adds contract-bound validation execution and aggregate evidence manifest.

### Boundary Conclusion

- semantic authority changed: NO
- ledger discipline preserved: YES
- evidence-manifest orchestration added: YES
- admission verdict still future work: YES

### Next Gate

- recommended next gate: Scope/Diff Policy Check Seed
- reason: validation orchestration now exists, but CCL still needs policy over what scope/diff is admissible before any verdict layer

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

- At the time of PR #2, checked-in Local Admission Guard and CCL capture were not implemented yet.
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
- Branch: feat/command-evidence-capture-seed-codex
- PR: #6 — https://github.com/skulmakov-oss/CCL/pull/6
- Merge commit: 84a81f970b581dd3e4e3fa3cf44c4e46596f9e12
- Final main HEAD after merge: 84a81f970b581dd3e4e3fa3cf44c4e46596f9e12

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
- ci/admission.ps1
- ci/admission.sh
- ci/common.sh
- ci/env_check.sh
- ci/rust_gate.sh
- ci/semantic_gate.sh

Edited:
- .gitignore
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
- `cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- powershell.exe -File .\ci\admission.ps1 --full`: PASS
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- GitHub CI used as evidence: NO

### Capture Proof

- cargo-version capture: PASS
- artifact path: `.ccl/runs/1781963867501-18332/commands/001-cargo-version/result.json`
- Local Admission Guard capture: PASS
- exact command: `cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- powershell.exe -File .\ci\admission.ps1 --full`
- artifact path: `.ccl/runs/1781964869831-19912/commands/001-local-admission-guard/result.json`
- capture artifact shape:
  - `.ccl/runs/<run-id>/run.json`
  - `.ccl/runs/<run-id>/evidence-manifest.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/command.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/env.json`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/stdout.txt`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/stderr.txt`
  - `.ccl/runs/<run-id>/commands/001-<command-id>/result.json`
- Local Admission Guard executed through CCL capture: YES
- Local Admission Guard capture result: PASS
- streaming stdout/stderr: YES
- output byte limits enforced: YES
- timeout stream drain bounded: YES
- backpressure/deadlock test result: PASS

### Warnings

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
