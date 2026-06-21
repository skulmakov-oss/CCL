# CCL Demo

## Purpose

This demo shows the current deterministic CCL gate pipeline from the repository root with one repeatable local command.

## What This Demo Proves

- CCL can check the admission contract locally.
- CCL can run repository preflight locally.
- CCL can execute the current gate pipeline and write manifests under `.ccl/runs/`.
- The demo uses captured local evidence-producing commands, not GitHub CI.

## What This Demo Does Not Prove

- It does not prove admission by itself.
- It does not replace CCL gate output.
- It does not prove sandboxing, manifest signing, or environment allowlist enforcement.
- It does not make external review or agent testimony admissible evidence.

## Prerequisites

- Run from the repository root.
- PowerShell available on Windows.
- Local Rust toolchain installed.

## Run

### Windows PowerShell

```powershell
.\scripts\demo.ps1
.\scripts\demo.ps1 -VerboseEvidence
```

### Git Bash / Linux / macOS

```bash
bash scripts/demo.sh
bash scripts/demo.sh --verbose-evidence
```

## Verbose Evidence Mode

Verbose mode also shows:

- validation runner;
- scope check;
- ledger verification.

## Expected Output

The script prints step headings and ends with a clear completion message.

The authoritative result is the CCL gate status printed by `ccl gate run`.

## Generated Artifacts

The demo produces local evidence artifacts under:

```text
.ccl/runs/
```

These artifacts are runtime output and are not committed.

## Troubleshooting

- If the script fails immediately, run it from the repository root.
- If `cargo` commands fail, verify the local Rust toolchain.
- If the gate fails, inspect the manifests under `.ccl/runs/`.

## Security Boundary

The demo is not evidence by itself.
The demo only runs CCL commands that produce evidence.
GitHub CI is not used as evidence.
Agent testimony is not used as evidence.
The authoritative output is the CCL gate result and generated manifests under `.ccl/runs/`.
