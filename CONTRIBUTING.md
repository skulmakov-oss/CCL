# Contributing to CCL

CCL is built around controlled, evidence-based engineering.

Contributions should preserve the core rule:

> No evidence, no PASS.

## Contribution Rules

- Keep changes small and scoped.
- Prefer deterministic logic over agent interpretation.
- Do not treat AI confidence as evidence.
- Do not treat GitHub CI as final validation evidence.
- Preserve local validation and reproducibility.
- Update project ledger documents when the task type requires it.
- Document warnings instead of hiding them.

## Expected Pull Request Shape

A good PR should include:

- clear objective;
- changed file list;
- validation commands;
- local output summary;
- ledger impact;
- final verdict.

## Forbidden Without Explicit Approval

Do not change these without explicit task authorization:

- license terms;
- repository governance rules;
- validation semantics;
- task verdict semantics;
- dependencies;
- CI / workflow configuration.

## Validation Philosophy

The final question is not:

```text
Does the agent believe it is done?
```

The final question is:

```text
Can the result be admitted from captured evidence?
```
