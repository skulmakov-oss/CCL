# Release Manifest Dry Assembly

## Purpose

This seed assembles a local dry release manifest from already-captured CCL evidence.

## Current Status

CCL does not yet create official release manifests from this flow.
This document describes a local dry assembly step only.

## Command

```powershell
cargo run -p ccl-cli -- release manifest dry-assemble `
  --version 0.1.0 `
  --repo . `
  --dry-run-manifest .ccl/runs/<dry-run-id>/release-dry-run-manifest.json `
  --ledger-verification-manifest .ccl/runs/<release-ledger-id>/release-ledger-verification-manifest.json `
  --checksum-manifest .ccl/runs/<checksum-id>/release-checksum-manifest.json
```

## Input Manifests

The command consumes three local manifests:

- release dry-run manifest
- release ledger verification manifest
- release checksum manifest

## Version and Tag Binding

The requested version must match all three input manifests.
The derived tag is `vMAJOR.MINOR.PATCH`.

## Source Commit Binding

The source commit recorded in the dry-run manifest must match the source commit recorded in the ledger verification manifest and the checksum manifest.

## Checksum Binding

Checksum entries are copied into the dry manifest as local evidence bindings.
The dry manifest does not invent new file hashes.

## Evidence Manifest Binding

The dry manifest records the three input manifest paths so the release intent can be audited locally.

## Dry Manifest Shape

The dry manifest records:

- schema version
- kind
- project
- version
- tag
- source commit
- evidence manifest paths
- checksum entries
- no-release-side-effect policy flags
- status, warnings, and violations

## Release Boundary

This seed assembles a local dry release manifest.
It does not create an official release.
It does not create Git tags.
It does not create official release artifacts.

## GitHub CI Boundary

GitHub CI remains metadata only.
GitHub CI is not release evidence.
A green GitHub check cannot replace local CCL evidence.

## Failure Modes

The command fails when input manifests are missing, malformed, the wrong kind, or do not agree on version, tag, source commit, or no-side-effect policy.

## What This Does Not Do

This seed does not publish anything.
It does not create a GitHub Release.
It does not publish to crates.io.
It does not make the dry manifest authoritative for official release publication.

## Future Work

Future release work can use the dry manifest as input to a local release-candidate decision step and later official release publication gates.
