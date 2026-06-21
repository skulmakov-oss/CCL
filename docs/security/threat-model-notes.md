# CCL Threat Model Notes

These notes record current assumptions, mitigations, and deferred hardening items for the CCL MVP.

## Trust Boundaries

- The agent is untrusted for admission.
- External reviews are untrusted for admission.
- GitHub CI is metadata, not final evidence.
- Local CCL capture is the evidence source.
- The Task Contract is policy input.
- The ledger is project memory only after deterministic verification.

## Agent Testimony vs Evidence

- Agent testimony may explain work, but it does not admit work.
- External review may highlight risks and backlog items, but it does not admit work.
- CCL-captured evidence supports admission.
- Deterministic manifests and verified ledger entries support the verdict.

## Current Mitigations

- argv execution, no shell by default
- stdin null
- streaming stdout/stderr capture
- wall-timeout
- output byte limits
- SHA-256 hashes
- environment snapshot
- validation-run-manifest.json
- scope-check-manifest.json
- admission-verdict.json
- gate-run-manifest.json
- ledger-verification-manifest.json
- untracked file detection
- forbidden paths before allowed paths
- entry-local ledger verification
- GitHub CI not used as evidence

## Known Risks

- environment variable manipulation risk
- host-level process isolation is not complete sandboxing
- manifest files are not cryptographically signed
- local filesystem trust assumptions remain
- ledger verifier is marker-based, not natural-language semantic understanding
- Windows and Linux may diverge in process handling
- large repository diff performance still needs future testing
- environment policy is currently record-only: capture snapshots env but does not enforce allowlists yet
- environment allowlist enforcement is planned as a future hardening gate

## Deferred Hardening

- environment allowlist policy
- manifest signing
- process tree hardening on all OS targets
- sandbox runner profile
- threat model tests
- demo script for agent workflows
- external review intake procedure

## Non-goals for Current MVP

- LLM-based security decisions
- automatic code repair
- full sandbox
- MCP/Broker
- UI/Tauri
- CI-as-authority
- natural-language admission
