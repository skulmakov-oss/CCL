<p align="center">
  <h1 align="center">CCL</h1>
  <p align="center"><strong>Cerebral Control Layer</strong></p>
  <p align="center"><em>Deterministic governance for controlled AI-agent software engineering.</em></p>
</p>

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
- [`ledger/project-ledger.md`](ledger/project-ledger.md) — active project ledger.
- [`examples/semantic-task-contract.json`](examples/semantic-task-contract.json) — initial Semantic task contract example.
- [`examples/ccl-validation-task-contract.json`](examples/ccl-validation-task-contract.json) — validation runner example contract.
- [`examples/ccl-scope-task-contract.json`](examples/ccl-scope-task-contract.json) — scope/diff policy check example contract.
- [`examples/ccl-admission-task-contract.json`](examples/ccl-admission-task-contract.json) — admission verdict example contract.

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

Next implementation direction:

```text
Gate Orchestration Seed
```

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

Future admission runner:

```powershell
ccl validate run --contract examples/ccl-validation-task-contract.json --repo .
```

---

## Name

Official name: **CCL**  
Full form: **Cerebral Control Layer**  
Optional internal codename: **Cerebro**

---

## License

No license has been selected yet.
