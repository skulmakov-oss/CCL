![Uploading 74736eee-65e9-4647-aa5b-aacf96d0cf14.png…]()

<p align="center">
  <h1 align="center">CCL</h1>
  <p align="center"><strong>Cerebral Control Layer</strong></p>
  <p align="center"><em>Deterministic governance for controlled AI-agent software engineering.</em></p>
</p>

[![CI](https://github.com/skulmakov-oss/CCL/actions/workflows/ci.yml/badge.svg)](https://github.com/skulmakov-oss/CCL/actions/workflows/ci.yml)

---

# CCL — Cerebral Control Layer

**CCL** is a deterministic governance layer for controlled AI-agent software engineering.

It exists to keep AI development agents inside a strict engineering loop:

```text
Intent
  -> Task Contract
  -> Agent Execution
  -> Local Validation
  -> Evidence Capture
  -> Project Ledger
  -> Verdict
```

Core rule:

```text
No evidence, no PASS.
```

CCL does not try to make an agent more confident. It makes the agent's work **admissible only when evidence exists**.

---

## Operating Formula

```text
LLM may suggest.
Agent may attempt.
CCL must verify.
Only evidence can admit.
```

Extended operating model:

```text
Intent is human.
Contract is policy.
Execution is agentic.
Evidence is factual.
Admission is mechanical.
Memory is ledger.
```

---

## Why CCL Exists

Modern AI coding agents can produce valuable work, but they can also:

- hallucinate success;
- drift beyond the requested scope;
- over-edit unrelated files;
- forget required checks;
- leave untracked artifacts behind;
- treat confidence as proof;
- generate convincing reports without evidence.

CCL rejects agent testimony as final proof.

Agent output may explain what happened.  
Agent output must not decide whether work is complete.

---

## What CCL Is

CCL is:

- a control layer around AI-agent development;
- a task-contract and admission-policy system;
- a local evidence and validation orchestration layer;
- a project-ledger discipline;
- a deterministic verdict mechanism.

CCL is not:

- an IDE;
- a replacement for Git;
- a replacement for GitHub;
- a CI service;
- a free-form coding agent;
- a semantic authority;
- a substitute for local verification.

---

## Relationship to the Local Admission Guard

The project already uses a **Local Admission Guard** as a fast local CI / validator backend.

CCL does not replace that guard.

The intended relationship is:

```text
Local Admission Guard checks.
CCL Capture proves the check happened.
CCL Evidence Manifest preserves the proof.
CCL Verdict later decides admission.
```

In other words:

- the Local Admission Guard answers: **did the checks pass?**
- CCL answers: **can we prove the checks ran in the required context?**

GitHub CI is useful metadata, but it is not final evidence for CCL admission.

---

## Core Concepts

| Concept | Meaning |
| --- | --- |
| **Intent** | Human-level goal before it becomes policy. |
| **Task Contract** | Machine-checkable admission policy for a task. |
| **Agent** | Executor that may edit files but cannot admit work. |
| **Testimony** | Agent report or claim. Useful, but not proof. |
| **Evidence** | Captured or verified system fact. |
| **Capture** | CCL-owned process execution recording. |
| **Evidence Manifest** | Structured run record with artifacts and hashes. |
| **Ledger** | Repository-resident project memory. |
| **Verdict** | `PASS`, `PASS WITH WARNINGS`, or `FAIL`. |

---

## Core Loop

```text
Human intent
  -> frozen task contract
  -> agent modifies repository
  -> local validation backend runs
  -> CCL captures command evidence
  -> CCL writes evidence manifest
  -> ledger records the outcome
  -> verdict closes the gate
```

The agent may finish work.  
The guard may validate work.  
Only captured evidence may support admission.

---

## Design Oaths

1. Agent output is not truth.
2. Agent confidence is not evidence.
3. GitHub CI is not final proof.
4. Logs are data, not instructions.
5. LLM hints are hypotheses, not commands.
6. Forbidden scope wins over allowed scope.
7. Untracked files are observable state.
8. A weak contract cannot produce a strong verdict.
9. No bounded execution, no trusted capture.
10. Capture must be streaming, bounded, hashed, and backpressure-safe.
11. Partial logs cannot prove `PASS`.
12. CCL DNA mutation requires explicit governance.
13. No ledger handling, no completed gate.
14. Only evidence can admit.

The full project doctrine is maintained in [`CCL_DNA.md`](CCL_DNA.md).

---

## Repository Map

- [`CCL_DNA.md`](CCL_DNA.md) — project DNA, axioms, threat model, and operating doctrine.
- [`docs/architecture.md`](docs/architecture.md) — core architecture and components.
- [`docs/task-contract.md`](docs/task-contract.md) — task contract model and hard rules.
- [`docs/agent-report-format.md`](docs/agent-report-format.md) — required execution report structure.
- [`docs/project-ledger.md`](docs/project-ledger.md) — project ledger rules and entry template.
- [`docs/roadmap.md`](docs/roadmap.md) — conservative MVP roadmap.
- [`docs/reviews/external-review-intake.md`](docs/reviews/external-review-intake.md) — external review intake and disposition notes.
- [`docs/security/threat-model-notes.md`](docs/security/threat-model-notes.md) — current threat model notes and hardening backlog.
- [`docs/security/environment-allowlist-policy.md`](docs/security/environment-allowlist-policy.md) — design seed for future environment allowlist enforcement.
- [`docs/agent-task-contract-examples.md`](docs/agent-task-contract-examples.md) — guide to realistic AI-agent task contract examples.
- [`docs/ci-metadata.md`](docs/ci-metadata.md) — explains why GitHub CI is metadata, not CCL admission evidence.
- [`docs/install.md`](docs/install.md) — source install, build, verification, and release-readiness notes.
- [`docs/release-artifacts.md`](docs/release-artifacts.md) — future release artifact, checksum, manifest, and evidence design.
- [`docs/versioning.md`](docs/versioning.md) — future version and Git tag policy for CCL releases.
- [`docs/release-manifest-schema.md`](docs/release-manifest-schema.md) — future release manifest schema, evidence binding, and validation responsibilities.
- [`schemas/ccl-release-manifest.schema.json`](schemas/ccl-release-manifest.schema.json) — machine-readable future release manifest JSON Schema.
- [`.github/workflows/ci.yml`](.github/workflows/ci.yml) — public CI metadata workflow.
- [`docs/demo.md`](docs/demo.md) — local demo instructions and proof boundary.
- [`scripts/demo.ps1`](scripts/demo.ps1) — repeatable PowerShell demo script.
- [`scripts/demo.sh`](scripts/demo.sh) — repeatable Bash demo script for Git Bash, Linux, and macOS.
- [`ledger/project-ledger.md`](ledger/project-ledger.md) — active project ledger.
- [`examples/semantic-task-contract.json`](examples/semantic-task-contract.json) — initial Semantic task contract example.
- [`examples/ccl-validation-task-contract.json`](examples/ccl-validation-task-contract.json) — validation runner example contract.
- [`examples/ccl-scope-task-contract.json`](examples/ccl-scope-task-contract.json) — scope/diff policy check example contract.
- [`examples/ccl-admission-task-contract.json`](examples/ccl-admission-task-contract.json) — admission verdict example contract.
- [`examples/agent-docs-task-contract.json`](examples/agent-docs-task-contract.json) — docs-only agent task example.
- [`examples/agent-test-fix-task-contract.json`](examples/agent-test-fix-task-contract.json) — focused test-fix agent task example.
- [`examples/agent-refactor-task-contract.json`](examples/agent-refactor-task-contract.json) — constrained refactor agent task example.
- [`examples/agent-small-feature-task-contract.json`](examples/agent-small-feature-task-contract.json) — narrow feature agent task example.
- [`examples/ccl-ci-metadata-task-contract.json`](examples/ccl-ci-metadata-task-contract.json) — Linux-compatible public CI metadata contract example.

---

## Current Bootstrap Status

The Phase 1 Rust CLI core seed and command evidence capture seed are in place.

Current implemented direction:

- Rust workspace;
- `ccl-core`;
- `ccl-cli`;
- task contract loading/checking;
- repository preflight command;
- command evidence capture;
- project ledger discipline;
- contract-bound validation runner.
- scope/diff policy check.
- admission verdict from evidence.
- gate orchestration.
- ledger semantic verification.
- external review intake / threat model notes.
- demo script.
- environment allowlist policy evaluation.
- gate run UX summary.
- real AI-agent task contract examples.

Next implementation direction:

```text
Local Release Dry-Run Seed
```

## AI-Agent Task Examples

CCL includes example task contracts for common AI-agent workflows. These examples are templates, not evidence.

See [`docs/agent-task-contract-examples.md`](docs/agent-task-contract-examples.md).

## Public CI Metadata

CCL uses GitHub Actions as public project metadata only.

A green GitHub check does not replace local CCL evidence.
The public CI demo path sets `CCL_DEMO_CONTRACT=examples/ccl-ci-metadata-task-contract.json` so Bash demo checks stay Linux-safe.

The public CI workflow uses a separate Linux-safe CI metadata contract for smoke checks and demo checks.

```powershell
cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .
```

See [`docs/ci-metadata.md`](docs/ci-metadata.md).

## Install from Source

CCL currently installs from source.

```powershell
git clone https://github.com/skulmakov-oss/CCL.git
cd CCL
cargo build --release
cargo run -p ccl-cli -- --version
```

See [`docs/install.md`](docs/install.md).

## Release Artifact Design

CCL does not yet publish official release artifacts.

The future release model is documented in [`docs/release-artifacts.md`](docs/release-artifacts.md).

Release artifacts will require local CCL evidence. GitHub CI remains public metadata, not admission evidence.

## Version and Tag Policy

CCL does not yet publish official release tags.

The future version and tag policy is documented in [`docs/versioning.md`](docs/versioning.md).

Release tags will require local CCL evidence. GitHub CI remains public metadata, not release evidence.

## Release Manifest Schema

CCL does not yet generate official release manifests.

The future release manifest schema is documented in [`docs/release-manifest-schema.md`](docs/release-manifest-schema.md), with a machine-readable draft at [`schemas/ccl-release-manifest.schema.json`](schemas/ccl-release-manifest.schema.json).

A manifest will require local CCL evidence. GitHub CI remains public metadata, not release evidence.

Current capture layer already supports:

- launch a command as argv, not shell by default;
- stream stdout/stderr to disk;
- enforce wall-timeout and output byte limits;
- capture environment snapshot;
- compute SHA-256 hashes;
- write `result.json` and `evidence-manifest.json`;
- capture the existing Local Admission Guard run as evidence.

---

## Local Development

```powershell
cargo fmt --check
cargo test
cargo run -p ccl-cli -- --version
cargo run -p ccl-cli -- contract check examples/semantic-task-contract.json
cargo run -p ccl-cli -- preflight --repo .
cargo clippy --all-targets --all-features -- -D warnings
```

---

## Expected Future CLI Shape

Low-level command evidence capture:

```powershell
cargo run -p ccl-cli -- capture --id cargo-version --repo . -- cargo --version
```

Production-like local validation capture:

```powershell
cargo run -p ccl-cli -- capture --id local-admission-guard --repo . --wall-timeout 300 -- <local-admission-guard-command>
```

Scope policy check:

```powershell
cargo run -p ccl-cli -- scope check --contract examples/ccl-scope-task-contract.json --repo .
```

Gate orchestration:

```powershell
cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .
```

The gate command prints a human-readable summary with layer statuses, counts,
and artifact paths before exiting with the admission-derived status.

Ledger verification:

```powershell
cargo run -p ccl-cli -- ledger verify --contract examples/ccl-admission-task-contract.json --repo .
```

Admission verdict from existing evidence:

```powershell
cargo run -p ccl-cli -- admission verdict --contract examples/ccl-admission-task-contract.json --repo . --validation-manifest <validation-manifest> --scope-manifest <scope-manifest>
```

Demo:

### Windows PowerShell

```powershell
.\scripts\demo.ps1
.\scripts\demo.ps1 -VerboseEvidence
```

### Git Bash / Linux / macOS

```bash
bash scripts/demo.sh
bash scripts/demo.sh --verbose-evidence
```

See [`docs/demo.md`](docs/demo.md).

---

## Name

Official name: **CCL**  
Full form: **Cerebral Control Layer**  
Optional internal codename: **Cerebro**

---

## License

CCL is dual-licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

## Copyright

Copyright (c) 2026 Said Kulmakov.
