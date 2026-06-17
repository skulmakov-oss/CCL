# Task Contract

A task contract is the primary unit of work in CCL.

It tells the agent what to do, what context to read, which files are allowed, which files are forbidden, how to validate the result, and what must be reported before the task can be accepted.

## Minimal Contract Fields

```json
{
  "project": "Semantic",
  "workstream": "R12 UI",
  "task_type": "source_pr",
  "objective": "Implement the next minimal UI carrier gate",
  "required_context": {
    "dna": true,
    "agent_skills": true,
    "latest_prs": 10,
    "project_ledger": true
  },
  "allowed_paths": [
    "crates/prom-ui/**",
    "docs/roadmap/post_ui/**"
  ],
  "forbidden_paths": [
    "Cargo.toml",
    "Cargo.lock",
    ".github/**",
    "docs/DNA.md",
    "docs/dna/**"
  ],
  "required_validation": [
    "git diff --check",
    "cargo fmt --check",
    "cargo test -p prom-ui",
    "local_admission_guard"
  ],
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}
```

## Task Types

| Task type | Meaning | Ledger required |
| --- | --- | --- |
| `audit_only` | Inspect and report without mutation | Optional / explicit exemption |
| `source_pr` | Implement a scoped source change | Yes |
| `closeout_pr` | Close a roadmap or gate item | Yes |
| `test_gate` | Add or adjust tests only | Yes |
| `guard_gate` | Change validation or Admission Guard logic | Yes, strict |
| `docs_gate` | Update documentation / roadmap | Yes |

## Hard Rules

- No final `PASS` without local validation.
- No completed gate without ledger handling.
- GitHub CI must not be final evidence.
- Changed files must match allowed scope.
- Forbidden files require explicit task authorization.
- Agent confidence must never override evidence.

## Verdict Inputs

A final verdict should be computed from:

- task contract;
- changed files;
- validation outputs;
- Admission Guard result;
- report completeness;
- ledger state;
- warning classification.
