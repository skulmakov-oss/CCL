# Public CI Metadata

## Purpose

This document explains the public GitHub Actions workflow for CCL and the boundary around it.

## What GitHub CI Checks

- basic Rust formatting and test health;
- contract validation for the checked-in example task contracts;
- a local CCL gate smoke check;
- the repeatable Bash demo flow;
- `clippy` on all targets and features.

## What GitHub CI Does Not Prove

- it does not replace local CCL evidence;
- it does not decide admission;
- it does not override `ccl gate run`;
- it does not make agent testimony admissible evidence;
- it does not prove sandboxing, signing, or policy hardening.

## CCL Authority Boundary

GitHub CI is public project metadata.
GitHub CI is not final CCL evidence.
A green GitHub check cannot replace local `ccl gate run`.

## Local Evidence Requirement

Admission remains local and evidence-bound.
Only captured local CCL artifacts and manifests can support a verdict.

## Recommended Maintainer Workflow

1. Use GitHub CI as a visible health signal.
2. Run `ccl gate run` locally before accepting work.
3. Review the generated manifests under `.ccl/runs/`.
4. Treat CI as metadata and CCL as the admission authority.

## Common Mistakes

- treating a green GitHub check as final proof;
- using CI output as admission evidence;
- conflating review/testimony with local captured evidence;
- assuming CI replaces local gate execution.

## Future Work

- branch protection and policy automation;
- additional platform coverage;
- artifact publishing decisions;
- release packaging and install guidance.
