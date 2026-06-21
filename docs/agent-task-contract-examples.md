# AI-Agent Task Contract Examples

## Purpose

This page provides realistic task contract templates for common AI-agent workflows.
The goal is to show how CCL can frame work before an agent starts editing files.

## What These Examples Are

- Templates for human-authored task contracts.
- Starting points for docs-only, test-fix, refactor, and small-feature work.
- Examples of contract boundaries that can be adapted to a specific task.

## What These Examples Are Not

- Evidence.
- Admission authority.
- A substitute for local validation.
- A substitute for CCL gate output.

## Example Catalog

| Example | Use Case | Risk Level | Intended Agent Work |
| --- | --- | --- | --- |
| `agent-docs-task-contract.json` | docs-only | low | documentation edits |
| `agent-test-fix-task-contract.json` | test fix | medium | focused bug/test fix |
| `agent-refactor-task-contract.json` | refactor | medium/high | behavior-preserving refactor |
| `agent-small-feature-task-contract.json` | feature | high | narrow feature implementation |

Risk level describes expected review strictness, not permission to bypass evidence.

## How to Use With an AI Coding Agent

1. Human chooses a task contract example.
2. Human adapts the objective, allowed paths, and validation commands.
3. Agent works only inside the contract boundary.
4. Human runs the local CCL gate.
5. CCL captures evidence.
6. CCL checks validation, scope, ledger, and environment policy.
7. CCL produces the admission verdict.
8. Human reviews the result and merges only after local evidence passes.

Agent may attempt.
CCL must verify.
Only evidence can admit.

## How to Validate an Example Contract

Start with contract validation:

```powershell
cargo run -p ccl-cli -- contract check examples/agent-docs-task-contract.json
```

Then use the selected contract with the local gate:

```powershell
cargo run -p ccl-cli -- gate run --contract examples/agent-docs-task-contract.json --repo .
```

Adapt the contract to the specific task before running real work.

## Recommended Agent Workflow

1. Prepare a contract from one of the examples.
2. Freeze the scope and validation commands.
3. Let the agent work.
4. Run CCL locally.
5. Review the evidence artifacts and verdict.

## How to Choose an Example

- Use `agent-docs-task-contract.json` when the work is limited to documentation, roadmap, or other non-code edits.
- Use `agent-test-fix-task-contract.json` when a focused test or helper fix is needed and the scope is still narrow.
- Use `agent-refactor-task-contract.json` when the work should preserve behavior but may touch internal CCL implementation details.
- Use `agent-small-feature-task-contract.json` when a narrow feature or UX improvement is needed and the validation boundary is strict.

Start with the lowest-risk example that still matches the task. If the task needs broader scope, write a new contract rather than stretching the example beyond its boundary.

## Evidence Boundary

These examples are not evidence.
An agent report is testimony.
GitHub CI is not final evidence.
Only captured local CCL evidence can support admission.

## Common Mistakes

- Treating example contracts as completed evidence.
- Letting agents edit outside allowed paths.
- Using GitHub CI as a final authority.
- Forgetting to update validation commands after changing the task.

## Next Steps

- Adapt the examples to your own repository tasks.
- Combine them with local gate runs and project ledger updates.
- Use them as templates for future agent workflows.
