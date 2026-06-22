# Release Ledger Entry Verification

## Purpose

This document describes deterministic verification that a local release dry-run is recorded in the project ledger.

The verifier binds the dry-run manifest to an entry-local ledger record.

## Current Status

Release ledger entry verification is implemented for local dry-runs.

It does not create a release, a tag, artifacts, checksums, or a GitHub Release.

## Command

```powershell
cargo run -p ccl-cli -- release ledger verify `
  --version 0.1.0 `
  --repo . `
  --dry-run-manifest .ccl/runs/release-dry-run-1782109060798-5384/release-dry-run-manifest.json `
  --ledger ledger/project-ledger.md
```

An explicit entry heading may be supplied when needed:

```powershell
cargo run -p ccl-cli -- release ledger verify `
  --version 0.1.0 `
  --repo . `
  --dry-run-manifest .ccl/runs/release-dry-run-1782109060798-5384/release-dry-run-manifest.json `
  --ledger ledger/project-ledger.md `
  --entry-heading "## 2026-06-22 — Release Dry-Run v0.1.0"
```

## Release Dry-Run Ledger Entry

The verified ledger entry must describe the release dry-run itself.

It is not an official release entry.

## Required Markers

The matched ledger entry must contain, in the same entry:

- `Status: PASS` or `Status: PASS WITH WARNINGS`;
- `Version: 0.1.0`;
- `Tag: v0.1.0`;
- `Source commit: <dry-run source commit>`;
- `Release dry-run manifest: <dry-run manifest path>`;
- `Local CCL gate status: PASS`;
- `GitHub CI used as evidence: NO`;
- `Tag created: NO`;
- `Release artifacts created: NO`;
- `Checksums generated: NO`;
- `GitHub Release created: NO`;
- `crates.io publish: NO`.

## Entry-Local Verification

Verification is entry-local.

Markers spread across multiple ledger entries do not satisfy the check.

## Dry-Run Manifest Binding

The verifier reads the dry-run manifest and binds the ledger entry to its version, tag, source commit, gate status, and evidence boundary.

## Source Commit Binding

The ledger entry must name the same source commit recorded in the dry-run manifest.

## Local CCL Gate Binding

The ledger entry must show the dry-run gate status as `PASS`.

## GitHub CI Boundary

GitHub CI is public metadata.

GitHub CI is not release evidence.

`GitHub CI used as evidence: NO` must appear in the verified entry.

## What This Does Not Do

This verifier does not create a release.

It does not create a Git tag.

It does not generate artifacts.

It does not generate checksums.

It does not publish a GitHub Release.

It does not publish to crates.io.

## Failure Modes

Typical failure modes include:

- missing ledger entry;
- version mismatch;
- tag mismatch;
- source commit mismatch;
- dry-run manifest mismatch;
- gate status mismatch;
- GitHub CI evidence violation;
- side-effect marker violation.

## Future Work

- official release ledger entry verification;
- checksum generation;
- release manifest generation;
- release artifact generation;
- signed release metadata.
