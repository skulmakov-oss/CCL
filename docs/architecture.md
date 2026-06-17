# CCL Architecture

CCL is a deterministic control layer for AI-agent-driven software engineering.

It does not try to be smarter than the agent. It defines the operating frame in which agent work can be accepted.

## System Position

```text
Human Architect
  -> CCL
  -> Task Contract
  -> Agent
  -> Evidence Capture
  -> Local Admission Guard
  -> Project Ledger
  -> Verdict
```

## Core Components

### 1. Task Contract Builder

Creates a strict task packet for an agent.

A task contract defines:

- objective;
- workstream;
- allowed paths;
- forbidden paths;
- required context;
- required validation;
- report format;
- ledger requirements;
- final verdict rules.

### 2. Context Loader

Collects the repository state needed before execution:

- current branch;
- current commit;
- working tree status;
- latest commits / PR basis;
- DNA / roadmap / agent instructions;
- project ledger state;
- relevant source files.

### 3. Agent Harness

Constrains how an agent may operate.

The harness should make scope boundaries explicit and reject uncontrolled drift.

### 4. Evidence Collector

Captures proof of execution:

- command;
- working directory;
- exit code;
- stdout;
- stderr;
- changed files;
- diff summary;
- validation output;
- Admission Guard result.

### 5. Report Verifier

Checks whether the agent report matches the required contract.

It must detect:

- missing mandatory commands;
- missing local validation;
- missing ledger update;
- claims without evidence;
- GitHub CI used as final proof;
- changed files outside allowed scope.

### 6. Ledger Enforcer

Ensures project state is recorded inside the repository when the task type requires it.

No completed gate without either:

1. a ledger update; or
2. an explicit audit-only exemption.

### 7. Verdict Engine

Produces one of three statuses:

- `PASS`
- `PASS WITH WARNINGS`
- `FAIL`

The verdict must be based on evidence, not agent confidence.

## Design Rule

The CCL core must be boring, deterministic, and auditable.

AI may assist with wording, summarization, and next-gate suggestions, but AI must not own final admission.
