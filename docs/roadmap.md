# CCL Roadmap

This roadmap defines a conservative MVP path for the Cerebral Control Layer.

## Phase 0 — Repository Bootstrap

Goal: define the product identity and control philosophy.

Deliverables:

- README;
- architecture outline;
- task contract specification;
- agent report format;
- project ledger rules;
- MVP roadmap.

Completion criterion:

- repository has enough structure to generate the first implementation task.

## Phase 1 — CLI Core

Goal: create a local deterministic CCL core before any UI work.

Status: CLI core seed merged via PR #2; ledger closeout recorded separately.

Expected commands:

```text
ccl init
ccl preflight
ccl make-task
ccl capture
ccl verify-report
ccl verdict
```

Expected capabilities:

- load a task contract;
- run repository preflight;
- capture command outputs;
- store run artifacts;
- verify agent reports;
- classify PASS / PASS WITH WARNINGS / FAIL.

## Phase 2 — Evidence Store

Goal: persist execution evidence in a structured local format.

Expected artifact layout:

```text
.ccl/runs/<run-id>/
  task-contract.json
  preflight.log
  validation.log
  admission.log
  changed-files.txt
  diff.patch
  report.md
  verdict.json
```

Validation-run aggregation can extend this layout with a run-level manifest that references captured command artifacts.

## Phase 3 — Ledger Enforcement

Goal: ensure task completion is linked to repository-resident project memory.

Expected behavior:

- detect whether a ledger update is required;
- verify ledger entry presence;
- compare ledger claims with captured evidence;
- reject completion when ledger requirements are missing.

Scope/diff policy checking, admission verdict from evidence, and gate orchestration are the prerequisite fence before ledger enforcement can trust a working tree as admissible.

## Phase 4 — Tauri Desktop Shell

Goal: provide a practical desktop interface over the deterministic core.

Expected panels:

- repository state;
- task contract builder;
- agent prompt output;
- command execution log;
- evidence viewer;
- ledger status;
- verdict panel;
- next gate recommendation.

## Phase 5 — Agent Integration

Goal: integrate external coding agents without giving them final authority.

Possible integrations:

- manual copy/paste task contracts;
- Codex CLI / environment integration;
- Antigravity workflow integration;
- GitHub PR metadata integration.

## Permanent Non-negotiables

- No evidence, no PASS.
- No ledger handling, no completed gate.
- No local validation, no final trust.
- GitHub CI is not final evidence.
- AI may assist; CCL decides through deterministic evidence.
