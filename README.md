# CCL — Cerebral Control Layer

**CCL** is a deterministic governance layer for controlled AI-agent software engineering.

It is designed to keep development agents inside a strict engineering loop: task contract, scoped execution, evidence capture, local validation, project ledger update, and final verdict.

> Core rule: **No evidence, no PASS.**

## Purpose

Modern AI coding agents can produce valuable work, but they can also drift, over-edit, rely on unverifiable claims, or treat confidence as proof. CCL exists to prevent that.

CCL does not replace the developer, Git, CI, or the project verifier. It controls the operational path by which an agent is allowed to claim that work is complete.

## Core Loop

```text
Intent
  -> Task Contract
  -> Agent Execution
  -> Evidence Capture
  -> Local Admission Guard
  -> Project Ledger
  -> Final Verdict
```

## Principles

- Agent output is not truth.
- Agent confidence is not evidence.
- GitHub CI is metadata, not final proof.
- Local validation is mandatory for trust.
- No ledger update means no completed gate.
- The control layer must be deterministic, boring, and auditable.
- AI may assist with interpretation, but it must not own the verdict.

## Initial Scope

The first CCL prototype is expected to focus on:

- task contract generation;
- repository preflight checks;
- required command checklists;
- execution report verification;
- project ledger enforcement;
- local Admission Guard integration;
- PASS / PASS WITH WARNINGS / FAIL verdict classification.

## Non-goals

CCL is not:

- an IDE;
- a replacement for Git;
- a replacement for GitHub;
- a CI service;
- a free-form coding agent;
- a semantic authority;
- a substitute for local verification.

## Repository Status

This repository is in the bootstrap phase.

The current goal is to define the product identity, architecture skeleton, operating contract, and MVP roadmap before implementation begins.

## Name

Official name: **CCL**  
Full form: **Cerebral Control Layer**  
Optional internal codename: **Cerebro**
