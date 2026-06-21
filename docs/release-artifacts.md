# Release Artifact Design

## Purpose

This document defines the future CCL release artifact model.
It records what release artifacts may exist, what evidence is required before they are trusted, and what release gates must exist before any publication path is considered admissible.

## Current Release Status

CCL does not yet publish official release artifacts.
CCL does not yet publish official binaries.
CCL is not yet published to crates.io.
CCL does not yet provide signed checksums.
CCL does not yet provide signed release manifests.

This document designs future release artifacts.
It does not create or publish release artifacts.

## Artifact Trust Model

A release artifact is a distributable output intended for users.
A release artifact is not trusted because it exists.
A release artifact is trusted only when it is linked to captured local CCL evidence.

Release evidence is the local proof that release checks ran.
Release metadata is public project signal such as GitHub CI.

GitHub CI may inform release review.
GitHub CI is not release evidence.
Local CCL gate evidence remains the release admission basis.

The intended flow is:

```text
source commit
  -> local CCL release gate
  -> captured release evidence
  -> artifact build
  -> checksum manifest
  -> release ledger entry
  -> human release decision
```

An artifact built from an unverified tree is not admissible.
A green CI run does not make an artifact admissible.
A GitHub uploaded artifact is not automatically a CCL release artifact.

## Candidate Artifact Types

| Artifact | Example Name | Status | Required Evidence |
| --- | --- | --- | --- |
| Source archive | `ccl-source-vX.Y.Z.tar.gz` | future | clean tag + local CCL gate |
| Windows binary | `ccl-cli-vX.Y.Z-windows-x86_64.zip` | future | platform build + checksum |
| Linux binary | `ccl-cli-vX.Y.Z-linux-x86_64.tar.gz` | future | platform build + checksum |
| macOS binary | `ccl-cli-vX.Y.Z-macos-aarch64.tar.gz` | future | platform build + checksum |
| Checksums | `SHA256SUMS` | future | generated from final artifacts |
| Manifest | `ccl-release-manifest.json` | future | references artifacts, checksums, and evidence |
| Release notes | `RELEASE_NOTES.md` | future | human-readable summary |
| Release ledger entry | project ledger release row | future | local CCL release gate + human decision |

## Non-Artifact Outputs

These outputs are not release artifacts by themselves:

- `.ccl/runs/**`
- GitHub Actions logs
- GitHub Actions temporary artifacts
- Codex or agent reports
- pull request descriptions
- local stdout/stderr pasted into comments

They may be evidence inputs or metadata, but they are not user-facing release artifacts unless a future release gate packages them into a release record.

## Required Evidence Before Release

Future release admission should require:

- clean working tree;
- source commit SHA;
- local CCL gate PASS;
- `cargo fmt --check`;
- `cargo test`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- contract checks;
- demo scripts;
- release artifact manifest generated;
- checksums generated;
- ledger release entry;
- GitHub CI checked as metadata only.

GitHub CI used as evidence: NO.
Local CCL release gate used as evidence: YES.

## Artifact Manifest Design

Future release gating can use a release manifest like:

```json
{
  "schema_version": 1,
  "project": "CCL",
  "version": "vX.Y.Z",
  "source_commit": "<sha>",
  "source_ref": "refs/tags/vX.Y.Z",
  "created_unix_ms": 0,
  "artifacts": [
    {
      "name": "ccl-cli-vX.Y.Z-windows-x86_64.zip",
      "kind": "binary_archive",
      "platform": "windows-x86_64",
      "sha256": "<sha256>",
      "size_bytes": 0
    }
  ],
  "evidence": {
    "local_gate_status": "PASS",
    "gate_manifest": ".ccl/runs/<id>/gate-run-manifest.json",
    "validation_manifest": ".ccl/runs/<id>/validation-run-manifest.json",
    "admission_verdict": ".ccl/runs/<id>/admission-verdict.json",
    "github_ci_used_as_evidence": false
  },
  "ledger": {
    "entry_required": true,
    "entry_heading": "## YYYY-MM-DD — Release vX.Y.Z"
  }
}
```

This is a proposed future shape only.
Do not generate it in this PR.

## Checksum Design

SHA-256 is the default checksum algorithm for future CCL release artifacts.
Checksums must be generated after final artifact bytes exist.
Checksums must be recorded in the release manifest.
Checksums must not be generated from intermediate build outputs unless those exact bytes are released.

Suggested future checksum files:

- `SHA256SUMS`
- `SHA256SUMS.txt`
- `ccl-release-manifest.json`

## Signature / Signing Status

Artifact signing is not implemented yet.
Signed manifests are future work.
Signed checksums are future work.

No trust claim should depend on signatures until signing exists.

Possible future signing options include:

- GPG signatures;
- cosign;
- Sigstore;
- signed release manifest.

## GitHub CI Boundary

GitHub CI is public metadata.
GitHub CI is not final release evidence.
GitHub CI artifacts are not automatically release artifacts.
A green GitHub check cannot replace local CCL release evidence.

## Local CCL Release Gate

The current local gate command is:

```powershell
cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .
```

That command is not a release publisher.
It is the current local evidence gate that future release packaging must build on.

Future release verification could introduce a dedicated command such as:

```powershell
cargo run -p ccl-cli -- release verify --manifest ccl-release-manifest.json
```

That future command is not implemented in this PR.

## Release Ledger Requirements

Future release ledger entries should record:

- version;
- source commit;
- source tag;
- artifact list;
- checksums;
- local CCL gate status;
- release manifest path;
- GitHub CI used as evidence: NO;
- human release decision;
- known warnings.

No release ledger automation exists yet.

## Platform Matrix

| Platform | Target | Artifact | Status |
| --- | --- | --- | --- |
| Windows | `x86_64-pc-windows-msvc` | zip | future |
| Linux | `x86_64-unknown-linux-gnu` | tar.gz | future |
| macOS Intel | `x86_64-apple-darwin` | tar.gz | future |
| macOS Apple Silicon | `aarch64-apple-darwin` | tar.gz | future |

This matrix is a release design target only.

## Future Release Gate Sequence

1. Release Artifact Design Seed
2. Version / Tag Policy Seed
3. Release Manifest Schema Seed
4. Local Release Dry-Run Seed
5. Checksum Generation Seed
6. GitHub Release Draft Seed
7. Signed Artifact Policy Seed

Recommended next gate:

Version / Tag Policy Seed

Reason:

Before artifact generation can be designed in detail, CCL needs deterministic version and tag rules.

## Non-goals for Current MVP

- GitHub Releases;
- release workflow;
- release artifact upload;
- binary archive generation;
- checksums generation;
- manifest signing;
- cosign signing;
- GPG signing;
- crates.io publishing;
- cargo publish;
- version bump;
- tag creation;
- branch protection;
- Docker image;
- Homebrew formula;
- Scoop manifest;
- Winget package;
- Nix flake;
- installer scripts;
- new Rust code;
- new CLI command;
- runtime behavior changes.

## Common Mistakes

- Treating a built file as trusted without local evidence.
- Treating GitHub CI as release evidence.
- Publishing artifacts before a local release gate exists.
- Generating checksums before final artifact bytes are fixed.
- Assuming upload implies admissibility.

## Future Work

- version and tag policy;
- release manifest schema;
- local release dry-run;
- checksum generation gate;
- GitHub release draft flow;
- signed artifact policy.
