# Release Checksum Generation

## Purpose

This document describes local SHA-256 checksum generation for explicitly selected files.

The command produces local checksum evidence for bytes that already exist in the repository.

## Current Status

CCL can generate local checksum evidence.

It does not create an official release.
It does not create Git tags.
It does not create official release artifacts.
It does not publish a GitHub Release.
It does not publish to crates.io.

## Command

```powershell
cargo run -p ccl-cli -- release checksum `
  --version 0.1.0 `
  --repo . `
  --input README.md `
  --input docs/release-dry-run.md
```

The command writes a local manifest under:

```text
.ccl/runs/release-checksum-<id>/release-checksum-manifest.json
```

## Explicit Inputs Only

Checksums are computed only for explicitly named input files.

Inputs are processed in the order given.
Duplicate inputs are preserved as separate evidence entries.

The command rejects:

- missing files;
- directories;
- path traversal;
- absolute paths;
- `.git/**`;
- `.ccl/runs/**`;
- symlinks;
- globs;
- recursive directory hashing;
- network paths.

## Raw Byte Hashing

The checksum is SHA-256 over the raw bytes stored on disk.

The command does not normalize line endings.
The command does not parse input files as UTF-8.
The command does not rewrite the bytes before hashing.

## Manifest Shape

The checksum manifest records:

- schema version and kind;
- project and release version/tag binding;
- repo path;
- source commit;
- algorithm;
- input path, size, and SHA-256 hash;
- policy flags showing that no release side effects occurred;
- warnings and violations, when present.

The manifest does not contain file contents.

## Source Commit Binding

The command records the current source commit from the repository HEAD.

The checksum evidence is tied to that source commit.

## Release Boundary

This seed generates local checksum evidence only.

It does not create an official release.
It does not create Git tags.
It does not create official release artifacts.
It does not publish a GitHub Release.
It does not publish to crates.io.

## GitHub CI Boundary

GitHub CI may inform review.
GitHub CI is not checksum evidence.
A green GitHub check cannot replace local CCL evidence.

## Failure Modes

Typical failure modes include:

- invalid version;
- no inputs;
- malformed input path;
- directory input;
- missing input file;
- path traversal;
- forbidden repo-local generated paths;
- source commit lookup failure;
- manifest write failure.

## What This Does Not Do

This command does not:

- verify final release bundles;
- build release artifacts;
- generate release manifests;
- create a Git tag;
- publish a GitHub Release;
- publish to crates.io;
- use GitHub CI as evidence.

## Future Work

- release manifest dry assembly;
- release artifact verification against checksums;
- release manifest validation;
- signed checksum policy;
- release publication flow.
