# Install and Build from Source

## Current Release Status

CCL does not yet publish official binaries.
CCL is not yet published to crates.io.
CCL does not yet provide package-manager installers.
The current installation path is from source.

GitHub CI is public metadata only.
Local CCL gate evidence remains the admission basis.

## Requirements

- Git
- Rust stable toolchain
- Cargo
- PowerShell for Windows demo and local admission commands
- Bash for Unix or Git Bash demo commands

## Clone the Repository

```powershell
git clone https://github.com/skulmakov-oss/CCL.git
cd CCL
```

## Build

Build the workspace in debug mode:

```powershell
cargo build
```

Build release artifacts:

```powershell
cargo build --release
```

Expected binaries:

- `target/debug/ccl-cli`
- `target/release/ccl-cli`

On Windows:

- `target\debug\ccl-cli.exe`
- `target\release\ccl-cli.exe`

## Run the CLI

Check the CLI version:

```powershell
cargo run -p ccl-cli -- --version
```

## Verify Contracts

Run the current contract checks:

```powershell
cargo run -p ccl-cli -- contract check examples/semantic-task-contract.json
cargo run -p ccl-cli -- contract check examples/ccl-validation-task-contract.json
cargo run -p ccl-cli -- contract check examples/ccl-scope-task-contract.json
cargo run -p ccl-cli -- contract check examples/ccl-admission-task-contract.json
cargo run -p ccl-cli -- contract check examples/ccl-env-policy-task-contract.json
cargo run -p ccl-cli -- contract check examples/agent-docs-task-contract.json
cargo run -p ccl-cli -- contract check examples/agent-test-fix-task-contract.json
cargo run -p ccl-cli -- contract check examples/agent-refactor-task-contract.json
cargo run -p ccl-cli -- contract check examples/agent-small-feature-task-contract.json
```

## Run the Local Gate

Use the local CCL gate from the repository root:

```powershell
cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .
cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo . --verbose
```

The gate writes local evidence artifacts under `.ccl/runs/`.
Those artifacts are local evidence and are intentionally not committed.

## Run Demo Scripts

Windows PowerShell:

```powershell
.\scripts\demo.ps1
.\scripts\demo.ps1 -VerboseEvidence
```

Bash:

```bash
bash scripts/demo.sh
bash scripts/demo.sh --verbose-evidence
```

## Windows Notes

- Use PowerShell for the demo script and local admission commands.
- `-ExecutionPolicy Bypass` may be needed when running scripts from a restrictive shell policy.
- The generated binary path uses `.exe` on Windows.

## Linux / macOS Notes

- Use Bash or a compatible shell.
- The `scripts/demo.sh` script is LF-normalized for Git Bash, Linux, and macOS.
- The release build output is under `target/release/ccl-cli`.

## Git Bash Notes

- Git Bash can run `bash scripts/demo.sh`.
- If line endings look wrong, check that Git is honoring LF for `scripts/demo.sh`.
- Git Bash still relies on local evidence from `ccl gate run`.

## Public CI vs Local Evidence

GitHub CI may inform review.
GitHub CI is not final CCL evidence.
A green GitHub check cannot replace local `ccl gate run`.
Local captured CCL manifests remain the admission basis.

## Troubleshooting

- Rust toolchain missing: install the Rust stable toolchain first.
- `cargo` not found: ensure Cargo is on `PATH`.
- PowerShell execution policy: run the script with `-ExecutionPolicy Bypass` if needed.
- Bash CRLF or line endings: use `bash scripts/demo.sh` and keep `scripts/demo.sh` LF-normalized.
- Clippy component missing: install the `clippy` component for the active toolchain.
- Local artifacts under `.ccl/runs`: they are expected and remain local evidence.

## Future Release Packaging

Future work may add:

- GitHub Releases;
- signed checksums;
- binary archives;
- crates.io publication;
- package manager manifests;
- release CI;
- manifest signing.

These are future work, not implemented release channels.

## Maintainer Release Checklist

Before creating any official release:

- [ ] Run local CCL gate.
- [ ] Run full local validation.
- [ ] Confirm working tree is clean.
- [ ] Confirm GitHub CI is green as metadata.
- [ ] Confirm GitHub CI is not used as admission evidence.
- [ ] Confirm version and tag policy.
- [ ] Generate release artifacts in a dedicated release PR.
- [ ] Generate checksums.
- [ ] Record release evidence in the project ledger.

This checklist is advisory until a dedicated release process exists.
