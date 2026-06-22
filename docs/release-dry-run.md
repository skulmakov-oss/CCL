# Local Release Dry-Run

## Purpose

This document describes the local CCL release dry-run command.
It validates release intent without publishing a release.

## Current Status

CCL does not yet publish official releases.
CCL does not yet create official Git release tags through this command.
CCL does not yet publish release artifacts from this command.
CCL does not yet publish to crates.io from this command.

The dry-run is local evidence only.
It is not a release.

## Command

```powershell
cargo run -p ccl-cli -- release dry-run --version 0.1.0 --repo .
```

An explicit contract may also be supplied:

```powershell
cargo run -p ccl-cli -- release dry-run --version 0.1.0 --repo . --contract examples/ccl-admission-task-contract.json
```

The default contract is `examples/ccl-admission-task-contract.json`.

## What the Dry-Run Checks

The dry-run checks:

- version shape;
- derived tag shape;
- repository cleanliness;
- release manifest schema file presence;
- release manifest schema JSON validity;
- the existing local CCL gate;
- local dry-run evidence writing.

## What the Dry-Run Does Not Do

The dry-run does not:

- create a tag;
- build release artifacts;
- generate checksums;
- publish a GitHub Release;
- publish to crates.io;
- sign anything;
- use GitHub CI as evidence.

## Dry-Run Manifest

The dry-run writes a local evidence manifest under:

```text
.ccl/runs/release-dry-run-<id>/release-dry-run-manifest.json
```

The manifest records version, derived tag, source commit, branch, tree cleanliness, schema checks, gate status, and policy boundaries.

It is not a release manifest.

## Version and Tag Handling

The dry-run accepts a SemVer-compatible version in `MAJOR.MINOR.PATCH` form.
It derives a tag by prefixing the version with `v`.

Example:

```text
version: 0.1.0
tag: v0.1.0
```

The dry-run validates both shapes.
It does not create the tag.

## Local CCL Gate Requirement

The dry-run reuses the existing local CCL gate.
It records the gate result in the dry-run manifest.

The local CCL gate remains the authority.

## GitHub CI Boundary

GitHub CI may inform review.
GitHub CI is not dry-run evidence.
A green GitHub check cannot replace the local CCL gate.

## Dirty Tree Policy

The dry-run requires a clean enough working tree for release intent.
Tracked changes or unignored local changes cause the dry-run to fail.

Local `.ccl/runs/**` evidence output is expected and does not count as release output.

## Relationship to Release Manifest Schema

The dry-run checks that `schemas/ccl-release-manifest.schema.json` exists and parses as JSON.

The dry-run does not generate the final release manifest.
The release manifest schema remains the future release authority for release packaging.

## Future Work

- release ledger entry verification;
- checksum generation;
- release manifest generation;
- release artifact generation;
- GitHub Release drafting;
- signed artifact policy.
