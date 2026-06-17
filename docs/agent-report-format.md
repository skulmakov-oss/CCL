# Agent Report Format

CCL requires agents to produce execution reports that are reconstructable and auditable.

A report is not a loose summary. It is an operational trace.

## Required Style

Use explicit action lines:

```markdown
Ran command: `git status --short --branch`

Ran command: `cargo fmt --check`

Used tool: <tool name>

Created <file path>

Edited <file path>
```

If no files were created:

```markdown
Created: none
```

If no files were edited:

```markdown
Edited: none
```

## Required Sections

```markdown
### Preflight Execution

### DNA / Roadmap / Skills Basis

### Recent PR / Commit Basis

### Implementation Execution

### Validation Execution

### PR / Merge Execution

### Post-merge Audit
```

If no PR or merge was performed:

```markdown
PR / Merge Execution:
Not performed.
```

## Final Verdict Block

````markdown
### Final Verdict

```text
Status:
PASS / PASS WITH WARNINGS / FAIL — <short verdict>

Preflight:
- repository state:
- current HEAD:
- working tree:
- branch mode:

DNA:
- files inspected:
- boundary conclusion:

Basis PRs / commits:
- latest audited:
- relevant PRs:
- dependency changes:
- DNA changes:
- agent skill changes:

Changed files:
- created:
- edited:
- deleted:

Validation:
- git diff --check:
- pr_body search:
- cargo fmt --check:
- cargo test target(s):
- local Admission Guard:
- GitHub CI used as evidence: NO

Project metadata:
- affected layer:
- semantic authority changed: YES / NO
- verifier boundary preserved: YES / NO
- dependency surface changed: YES / NO

Recommended next gate:
- next safest follow-up:
- reason:
- expected files:
- forbidden files:
- required validation:

Final status:
- task completed / not completed
- safe to proceed: YES / NO
```
````

## Invalid Report Conditions

A report is invalid if it:

- claims `PASS` without local validation;
- uses GitHub CI as final proof;
- omits changed files;
- omits required commands;
- hides warnings;
- invents evidence;
- fails to address the project ledger requirement.
