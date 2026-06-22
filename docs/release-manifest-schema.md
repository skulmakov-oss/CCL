# Release Manifest Schema

## Purpose

This document defines the future CCL release manifest schema.
It describes the machine-readable policy metadata that will bind version, tag, source commit, artifacts, checksums, local CCL evidence, ledger entry, and GitHub CI metadata boundary.

## Current Status

CCL does not yet generate official release manifests.
CCL does not yet publish official releases.
CCL does not yet publish official binaries.
This document defines future schema and validation policy only.

It does not generate manifests.
It does not validate final release artifacts by itself.
It does not make GitHub CI evidence.

## Schema File

The machine-readable schema lives at [`schemas/ccl-release-manifest.schema.json`](../schemas/ccl-release-manifest.schema.json).

## Manifest Trust Boundary

A release manifest is not trusted because it exists.
A release manifest is admissible only when it binds version, tag, source commit, final artifacts, checksums, local CCL evidence, and ledger entry.

GitHub CI is public metadata only.
GitHub CI is not release evidence.
A green GitHub check cannot replace local CCL release evidence.

## Top-Level Fields

The schema requires these top-level fields:

- `schema_version`
- `project`
- `version`
- `tag`
- `source`
- `artifacts`
- `checksums`
- `evidence`
- `ledger`
- `ci_metadata`
- `created_unix_ms`

## Source Binding

The `source` object binds the manifest to a specific release source identity:

- `commit`: 40 lowercase hex characters;
- `ref`: a `refs/tags/vMAJOR.MINOR.PATCH` reference;
- `tree_clean`: `true` for official release manifests.

## Artifact Entries

Each artifact entry must include:

- `name`
- `kind`
- `platform`
- `path`
- `sha256`
- `size_bytes`

Recommended `kind` values:

- `source_archive`
- `binary_archive`
- `release_notes`
- `checksum_file`
- `manifest`

Recommended `platform` values:

- `source`
- `windows-x86_64`
- `linux-x86_64`
- `macos-x86_64`
- `macos-aarch64`
- `portable`

Artifact paths should be relative and should not contain parent-directory traversal.
This seed does not implement path canonicalization.

## Checksum Entries

The `checksums` object requires:

- `algorithm`
- `entries`

For this seed, `algorithm` is `sha256`.
Each checksum entry binds:

- `artifact_name`
- `sha256`

Checksums must correspond to final artifact bytes.
Cross-array consistency between `artifacts` and `checksums` is future validator work.

## Evidence Binding

The `evidence` object binds the manifest to local CCL proof:

- `local_ccl_gate_status`
- `gate_manifest`
- `validation_manifest`
- `scope_manifest`
- `ledger_manifest`
- `admission_verdict`

Future official release manifests should require local gate PASS.
This seed allows the schema to express the evidence structure while leaving strict release-mode enforcement to a future validator.

## Ledger Binding

The `ledger` object requires:

- `entry_required`
- `entry_heading`

`entry_required` must be `true`.
`entry_heading` identifies the future ledger row that records the release decision.

No ledger handling exists in this PR.
No completed release gate exists in this PR.

## GitHub CI Metadata

The `ci_metadata` object records the public CI state:

- `github_ci_checked`
- `github_ci_status`
- `github_ci_used_as_evidence`

Valid `github_ci_status` values are:

- `PASS`
- `PASS_WITH_WARNINGS`
- `FAIL`
- `NOT_CHECKED`

`github_ci_used_as_evidence` must be `false`.
GitHub CI is metadata, not release evidence.

## What the Schema Does Not Prove

The schema does not prove that:

- artifact files exist;
- artifact bytes match hashes;
- referenced manifests exist;
- local CCL manifests belong to the same source commit;
- the ledger entry exists in the repository;
- the release has been approved by a human;
- GitHub CI is sufficient for release admission.

## Future Validator Responsibilities

A future release validator should:

- validate the schema;
- verify version/tag agreement;
- verify source commit matches the tagged commit;
- verify clean tree evidence;
- verify artifact files exist;
- verify artifact `sha256` values match final bytes;
- verify checksum entries match artifact entries;
- verify local CCL evidence manifests exist;
- verify evidence manifests refer to the same source commit;
- verify the ledger entry exists;
- verify GitHub CI used as evidence: `NO`.

## Example Manifest Shape

An example future manifest shape could look like this:

```json
{
  "schema_version": 1,
  "project": "CCL",
  "version": "0.1.0",
  "tag": "v0.1.0",
  "source": {
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "ref": "refs/tags/v0.1.0",
    "tree_clean": true
  },
  "artifacts": [
    {
      "name": "ccl-cli-v0.1.0-linux-x86_64.tar.gz",
      "kind": "binary_archive",
      "platform": "linux-x86_64",
      "path": "dist/ccl-cli-v0.1.0-linux-x86_64.tar.gz",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "size_bytes": 123456
    }
  ],
  "checksums": {
    "algorithm": "sha256",
    "entries": [
      {
        "artifact_name": "ccl-cli-v0.1.0-linux-x86_64.tar.gz",
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
      }
    ]
  },
  "evidence": {
    "local_ccl_gate_status": "PASS",
    "gate_manifest": ".ccl/runs/gate-<id>/gate-run-manifest.json",
    "validation_manifest": ".ccl/runs/validation-<id>/validation-run-manifest.json",
    "scope_manifest": ".ccl/runs/scope-<id>/scope-check-manifest.json",
    "ledger_manifest": ".ccl/runs/ledger-<id>/ledger-verification-manifest.json",
    "admission_verdict": ".ccl/runs/admission-<id>/admission-verdict.json"
  },
  "ledger": {
    "entry_required": true,
    "entry_heading": "## 2026-06-22 — Release v0.1.0"
  },
  "ci_metadata": {
    "github_ci_checked": true,
    "github_ci_status": "PASS",
    "github_ci_used_as_evidence": false
  },
  "created_unix_ms": 0
}
```

This is illustrative only.
It is not a real release manifest.

## Non-goals for Current MVP

- manifest generation;
- manifest validation CLI;
- release artifact generation;
- checksums generation;
- tag creation;
- version bumping;
- signing or verification workflows;
- runtime changes;
- GitHub Release publishing.

## Future Work

- Local Release Dry-Run Seed;
- Checksum Generation Seed;
- GitHub Release Draft Seed;
- Signed Artifact Policy Seed;
- release manifest validator implementation;
- release gate integration.
