# Project Ledger

The project ledger is the repository-resident memory of completed gates, audits, warnings, and next-step recommendations.

The ledger prevents agent work from disappearing into chat history.

## Purpose

The ledger records what changed in the project state after a task.

It should answer:

- what was done;
- why it was done;
- which evidence supports it;
- which warnings remain;
- what the next safest gate is.

## Ledger Requirement

No task is considered complete without either:

1. a project ledger update; or
2. a clear audit-only exemption.

## Entry Template

```markdown
## <YYYY-MM-DD> — <Gate / Task Name>

Status: PASS / PASS WITH WARNINGS / FAIL

### Scope

- Workstream:
- Task type:
- Branch:
- PR:
- Commit(s):

### Basis

- DNA files read:
- Roadmap files read:
- Agent skill files read:
- Recent PRs / commits audited:

### Changed Files

Created:
- ...

Edited:
- ...

Deleted:
- ...

### Validation

- `git diff --check`:
- `cargo fmt --check`:
- test target(s):
- local Admission Guard:
- GitHub CI used as evidence: NO

### Warnings

- ...

### Boundary Conclusion

- semantic authority changed: YES / NO
- verifier boundary preserved: YES / NO
- dependency surface changed: YES / NO

### Next Gate

- recommended next gate:
- reason:
- expected files:
- forbidden files:
```

## Rules

- The ledger must be factual.
- The ledger must not hide validation warnings.
- The ledger must not treat CI status as final proof.
- The ledger must preserve the relation between task, evidence, and verdict.
- The ledger should be short enough to scan and strict enough to audit.
