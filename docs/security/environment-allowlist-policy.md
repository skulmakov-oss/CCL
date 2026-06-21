# Environment Allowlist Policy Design

## Purpose

This document is a design seed for future environment variable policy in CCL command evidence capture.

It does not implement enforcement.

Current CCL capture records environment snapshots.
Future capture should support policy-bound environment allowlists.
Admission must eventually reject evidence produced under forbidden environment context.

## Design Status

This is a design-only specification.

It defines future policy behavior, manifest shape, and admission semantics.
It does not change runtime behavior in the current PR.

## Problem Statement

Environment variables can change tool behavior without changing command argv.
Agents may accidentally or intentionally influence validation by setting environment variables.
Some environment variables are harmless context.
Some are required for tools to run.
Some can suppress warnings, alter compiler behavior, disable tests, change cargo behavior, redirect paths, or inject credentials.
CCL currently snapshots environment, but does not yet enforce an environment policy.

Examples that must be classified, not blindly accepted or rejected:

- `RUSTFLAGS`
- `CARGO_ENCODED_RUSTFLAGS`
- `RUST_TEST_THREADS`
- `CARGO_HOME`
- `RUSTUP_HOME`
- `PATH`
- `HOME`
- `USERPROFILE`
- `CI`
- `GITHUB_ACTIONS`
- `NO_COLOR`
- `TERM`

## Threat Model

- Environment variables are part of execution context.
- Execution context may influence build, test, scope, and capture behavior.
- Secret-bearing variables may leak into manifests if not redacted.
- Platform-specific behavior makes env handling non-uniform across Windows, Linux, and macOS.
- Git Bash, PowerShell, and Unix shells may expose different inherited environments.

## Policy Goals

- Classify environment variables deterministically.
- Support a conservative allowlist with explicit deny precedence.
- Support record-only, warn, enforce, and strict modes.
- Redact secret-like values in manifests.
- Preserve enough context for later evidence review without exposing secrets.
- Make admission semantics explicit and deterministic.

## Environment Variable Classes

| Class | Meaning |
| --- | --- |
| `required_runtime` | Needed for OS/tool execution. |
| `toolchain_path` | Controls where tools are found or stored. |
| `toolchain_behavior` | Changes compiler or build behavior. |
| `test_behavior` | Changes test execution behavior. |
| `ci_metadata` | Indicates CI or platform context. |
| `display_only` | Affects colors or terminal formatting. |
| `credential_or_secret` | Contains auth or secret material. |
| `network_proxy` | Can redirect network access. |
| `unknown` | Not classified by policy. |

## Baseline Allowlist

A conservative baseline allowlist should be platform-aware.

### Required runtime

- `PATH`
- `SystemRoot`
- `WINDIR`
- `COMSPEC`
- `PATHEXT`
- `HOME`
- `USERPROFILE`
- `TMP`
- `TEMP`

### Display-only

- `NO_COLOR`
- `TERM`
- `CLICOLOR`

`PATH` is required but sensitive.
It should be recorded and may need normalization in future enforcement.

## Explicit Denylist

These variables should normally block admission unless explicitly allowed by contract:

- `RUSTFLAGS`
- `CARGO_ENCODED_RUSTFLAGS`
- `CARGO_TARGET_DIR`
- `RUST_TEST_THREADS`
- `CI`
- `GITHUB_ACTIONS`
- `GITHUB_TOKEN`
- `GH_TOKEN`
- `HTTP_PROXY`
- `HTTPS_PROXY`
- `ALL_PROXY`
- `NO_PROXY`

Deny precedence must win over allow precedence.

Unknown sensitive-looking variables should produce warnings or failure depending on policy mode.

## Recorded-only Variables

Some environment variables should be captured for traceability without automatically influencing admission.

Examples:

- `CI`
- `GITHUB_ACTIONS`
- `NO_COLOR`
- `TERM`

Recorded-only does not mean harmless.
It means the current policy may store or classify them without immediate hard failure in record-only mode.

## Task Contract Policy Shape

Future contracts may include a policy block similar to:

```json
{
  "environment_policy": {
    "mode": "enforce",
    "allow": [
      "PATH",
      "SystemRoot",
      "WINDIR",
      "HOME",
      "USERPROFILE",
      "TMP",
      "TEMP",
      "NO_COLOR",
      "TERM"
    ],
    "deny": [
      "RUSTFLAGS",
      "CARGO_ENCODED_RUSTFLAGS",
      "GITHUB_TOKEN",
      "GH_TOKEN"
    ],
    "allow_prefixes": [
      "CARGO_TERM_"
    ],
    "deny_prefixes": [
      "GITHUB_",
      "ACTIONS_"
    ],
    "redact_patterns": [
      "TOKEN",
      "SECRET",
      "PASSWORD",
      "KEY"
    ],
    "unknown": "warn"
  }
}
```

Rules:

- deny wins over allow
- explicit exact matches should take precedence over broader prefixes
- secret-like variables must be redacted in manifests
- policy mode controls admission effect

This is a proposed shape only.

## Capture Manifest Policy Shape

Future manifests may include a policy summary such as:

```json
{
  "environment_policy": {
    "mode": "enforce",
    "status": "PASS",
    "checked": true,
    "violations": [],
    "warnings": [],
    "redacted_variables": [
      "GITHUB_TOKEN"
    ],
    "allowed_variables_count": 12,
    "denied_variables_count": 0,
    "unknown_variables_count": 3
  }
}
```

Raw environment snapshots may contain sensitive values.
Future manifests should avoid exposing secret values.

Hashing and redaction strategy must be defined before enforcement.

## Admission Semantics

- environment policy PASS + validation PASS + scope PASS + ledger PASS => may admit PASS
- environment policy WARN => admission PASS_WITH_WARNINGS
- environment policy FAIL => admission FAIL
- environment policy CONTRACT_FAIL => admission CONTRACT_FAIL

Rules:

- GitHub CI metadata must not become admission evidence.
- Forbidden environment context invalidates captured command evidence for admission.
- If env policy is missing in enforce or strict phases, admission must not silently PASS.

## Failure Classes

Future failure classes may include:

- `env_forbidden_variable_present`
- `env_unknown_variable_present`
- `env_secret_value_unredacted`
- `env_policy_missing`
- `env_policy_contract_invalid`
- `env_snapshot_missing`
- `env_snapshot_hash_mismatch`

## Cross-platform Notes

- Windows environment variables are case-insensitive.
- Unix environment variables are case-sensitive.
- PATH separators differ.
- Git Bash may expose a mixed Windows and Unix environment.
- PowerShell and Bash demos may produce different environment snapshots.
- Policy must account for OS-specific baselines.

## Non-goals

- LLM-based security decisions
- automatic code repair
- full sandbox
- MCP/Broker
- UI/Tauri
- CI-as-authority
- natural-language admission

## Future Implementation Gates

- Environment Allowlist Enforcement Seed
- Manifest Signing Design Seed
- Process Isolation Hardening Plan
- Threat Model Expansion
