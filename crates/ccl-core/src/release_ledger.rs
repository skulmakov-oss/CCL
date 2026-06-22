use crate::release::{
    derive_release_tag, is_valid_release_tag, is_valid_release_version, ReleaseDryRunManifest,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReleaseLedgerVerificationRequest {
    pub repo: PathBuf,
    pub version: String,
    pub dry_run_manifest_path: PathBuf,
    pub ledger_path: PathBuf,
    pub entry_heading: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseLedgerVerificationStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ReleaseLedgerVerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseLedgerVerificationStatus::Pass => write!(f, "PASS"),
            ReleaseLedgerVerificationStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ReleaseLedgerVerificationStatus::Fail => write!(f, "FAIL"),
            ReleaseLedgerVerificationStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseLedgerVerificationManifest {
    pub schema_version: u32,
    pub kind: String,
    pub project: String,
    pub version: String,
    pub tag: String,
    pub ledger_path: String,
    pub dry_run_manifest_path: String,
    pub source_commit: String,
    pub gate_status: String,
    pub matched_entry_heading: String,
    #[serde(default)]
    pub required_markers: Vec<String>,
    pub status: ReleaseLedgerVerificationStatus,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub created_unix_ms: u128,
    pub github_ci_used_as_evidence: bool,
}

#[derive(Debug, Clone)]
pub struct ReleaseLedgerVerificationOutcome {
    pub status: ReleaseLedgerVerificationStatus,
    pub manifest: ReleaseLedgerVerificationManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseLedgerVerificationError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
struct LedgerEntry {
    heading: String,
    text: String,
}

pub fn run_release_ledger_verification(
    request: ReleaseLedgerVerificationRequest,
) -> Result<ReleaseLedgerVerificationOutcome, ReleaseLedgerVerificationError> {
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_id = generate_release_ledger_run_id();
    let run_dir = repo_root.join(".ccl").join("runs").join(&run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("release-ledger-verification-manifest.json");
    let created_unix_ms = system_time_ms(SystemTime::now());

    let version = request.version.clone();
    let tag = derive_release_tag(&version);
    let dry_run_manifest_path = repo_relative_string(&repo_root, &request.dry_run_manifest_path);
    let ledger_path = repo_relative_string(&repo_root, &request.ledger_path);

    let warnings = Vec::new();
    let mut violations = Vec::new();
    let mut required_markers = Vec::new();
    let mut status = ReleaseLedgerVerificationStatus::Pass;
    let mut matched_entry_heading = "N/A".to_string();
    let mut source_commit = String::new();

    if !is_valid_release_version(&version) {
        violations.push("release_ledger_version_invalid".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
    }

    if !is_valid_release_tag(&tag) {
        violations.push("release_ledger_tag_invalid".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
    }

    let dry_run_manifest = match read_dry_run_manifest(&repo_root, &request.dry_run_manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            violations.push(format!(
                "release_ledger_dry_run_manifest_missing: {}",
                error
            ));
            status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
            let manifest = finalize_manifest(ReleaseLedgerVerificationBuild {
                base: ReleaseLedgerVerificationBase {
                    ledger_path,
                    dry_run_manifest_path,
                    version,
                    tag,
                    created_unix_ms,
                },
                source_commit,
                gate_status: "N/A".to_string(),
                matched_entry_heading,
                required_markers,
                status,
                warnings,
                violations,
            });
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    if dry_run_manifest.schema_version != 1 {
        violations.push("release_ledger_dry_run_schema_version_mismatch".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
    }

    if dry_run_manifest.kind != "release_dry_run" {
        violations.push("release_ledger_dry_run_kind_mismatch".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
    }

    if dry_run_manifest.project != "CCL" {
        violations.push("release_ledger_project_mismatch".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
    }

    if dry_run_manifest.version != version {
        violations.push("release_ledger_version_mismatch".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    if dry_run_manifest.tag != tag {
        violations.push("release_ledger_tag_mismatch".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    if dry_run_manifest.source.head_commit.trim().is_empty() {
        violations.push("release_ledger_source_commit_missing".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    } else {
        source_commit = dry_run_manifest.source.head_commit.clone();
    }

    if !dry_run_manifest.source.tree_clean {
        violations.push("release_ledger_dry_run_tree_not_clean".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    if dry_run_manifest.policy.tag_created {
        violations.push("release_ledger_tag_created_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }
    if dry_run_manifest.policy.artifacts_created {
        violations.push("release_ledger_artifacts_created_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }
    if dry_run_manifest.policy.checksums_generated {
        violations.push("release_ledger_checksums_generated_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }
    if dry_run_manifest.policy.github_release_created {
        violations.push("release_ledger_github_release_created_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }
    if dry_run_manifest.policy.crates_io_published {
        violations.push("release_ledger_crates_io_published_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }
    if dry_run_manifest.policy.github_ci_used_as_evidence {
        violations.push("release_ledger_github_ci_evidence_violation".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    if dry_run_manifest.gate.gate_status.is_empty() {
        violations.push("release_ledger_gate_status_missing".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    } else if dry_run_manifest.gate.gate_status != "PASS" {
        violations.push("release_ledger_gate_status_not_pass".to_string());
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    let ledger_text = match fs::read_to_string(resolve_input_path(&repo_root, &request.ledger_path))
    {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("release_ledger_read_failed: {}", error));
            status = worst_status(status, ReleaseLedgerVerificationStatus::ContractFail);
            let manifest = finalize_manifest(ReleaseLedgerVerificationBuild {
                base: ReleaseLedgerVerificationBase {
                    ledger_path,
                    dry_run_manifest_path,
                    version,
                    tag,
                    created_unix_ms,
                },
                source_commit,
                gate_status: dry_run_manifest.gate.gate_status.clone(),
                matched_entry_heading,
                required_markers,
                status,
                warnings,
                violations,
            });
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };
    let entries = parse_entries(&ledger_text);
    let selected_entry = if let Some(explicit_heading) = request.entry_heading.as_deref() {
        entries
            .iter()
            .find(|entry| entry.heading.trim() == explicit_heading.trim())
            .cloned()
    } else {
        entries
            .iter()
            .find(|entry| entry.heading.contains("Release Dry-Run"))
            .cloned()
    };

    let selected_entry = match selected_entry {
        Some(entry) => entry,
        None => {
            violations.push("release_ledger_entry_missing".to_string());
            status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
            let manifest = finalize_manifest(ReleaseLedgerVerificationBuild {
                base: ReleaseLedgerVerificationBase {
                    ledger_path,
                    dry_run_manifest_path,
                    version,
                    tag,
                    created_unix_ms,
                },
                source_commit,
                gate_status: dry_run_manifest.gate.gate_status.clone(),
                matched_entry_heading,
                required_markers,
                status,
                warnings,
                violations,
            });
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    matched_entry_heading = selected_entry.heading.clone();

    let status_marker = field_value(&selected_entry.text, "Status");
    let entry_status = match status_marker.as_deref() {
        Some("PASS") => Some(ReleaseLedgerVerificationStatus::Pass),
        Some("PASS WITH WARNINGS") => Some(ReleaseLedgerVerificationStatus::PassWithWarnings),
        Some(other) => {
            violations.push(format!("release_ledger_status_mismatch: {}", other));
            status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
            None
        }
        None => {
            violations.push("release_ledger_status_missing".to_string());
            status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
            None
        }
    };
    if let Some(marker) = status_marker {
        required_markers.push(format!("Status: {}", marker));
    }
    if let Some(entry_status) = entry_status {
        status = worst_status(status, entry_status);
    }

    check_required_field(
        &selected_entry.text,
        "Version",
        &version,
        "release_ledger_version_mismatch",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Tag",
        &tag,
        "release_ledger_tag_mismatch",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Source commit",
        &source_commit,
        "release_ledger_source_commit_mismatch",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Release dry-run manifest",
        &dry_run_manifest_path,
        "release_ledger_dry_run_manifest_mismatch",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Local CCL gate status",
        "PASS",
        "release_ledger_gate_status_mismatch",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "GitHub CI used as evidence",
        "NO",
        "release_ledger_github_ci_evidence_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Tag created",
        "NO",
        "release_ledger_tag_created_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Release artifacts created",
        "NO",
        "release_ledger_artifacts_created_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "Checksums generated",
        "NO",
        "release_ledger_checksums_generated_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "GitHub Release created",
        "NO",
        "release_ledger_github_release_created_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );
    check_required_field(
        &selected_entry.text,
        "crates.io publish",
        "NO",
        "release_ledger_crates_io_published_violation",
        &mut required_markers,
        &mut violations,
        &mut status,
    );

    if dry_run_manifest.gate.gate_status != "PASS" {
        status = worst_status(status, ReleaseLedgerVerificationStatus::Fail);
    }

    let manifest = finalize_manifest(ReleaseLedgerVerificationBuild {
        base: ReleaseLedgerVerificationBase {
            ledger_path,
            dry_run_manifest_path,
            version,
            tag,
            created_unix_ms,
        },
        source_commit,
        gate_status: dry_run_manifest.gate.gate_status.clone(),
        matched_entry_heading,
        required_markers,
        status,
        warnings,
        violations,
    });
    write_manifest(&manifest_path, &manifest)?;
    Ok(outcome_from_manifest(&repo_root, manifest_path, manifest))
}

fn read_dry_run_manifest(
    repo_root: &Path,
    path: &Path,
) -> Result<ReleaseDryRunManifest, ReleaseLedgerVerificationError> {
    let content = fs::read_to_string(resolve_input_path(repo_root, path))?;
    Ok(serde_json::from_str(&content)?)
}

fn check_required_field(
    entry_text: &str,
    field: &str,
    expected: &str,
    violation_kind: &str,
    required_markers: &mut Vec<String>,
    violations: &mut Vec<String>,
    status: &mut ReleaseLedgerVerificationStatus,
) {
    match field_value(entry_text, field) {
        Some(value) if value == expected => {
            required_markers.push(format!("{}: {}", field, value));
        }
        Some(value) => {
            required_markers.push(format!("{}: {}", field, value.clone()));
            violations.push(violation_kind.to_string());
            *status = worst_status(status.clone(), ReleaseLedgerVerificationStatus::Fail);
        }
        None => {
            violations.push(violation_kind.to_string());
            *status = worst_status(status.clone(), ReleaseLedgerVerificationStatus::Fail);
        }
    }
}

fn field_value(entry_text: &str, field: &str) -> Option<String> {
    for line in entry_text.lines() {
        let trimmed = line.trim().trim_start_matches("- ").trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case(field) {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn parse_entries(text: &str) -> Vec<LedgerEntry> {
    let mut entries = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_lines = Vec::new();

    for line in text.lines() {
        if line.starts_with("## ") {
            if let Some(heading) = current_heading.take() {
                entries.push(LedgerEntry {
                    heading,
                    text: current_lines.join("\n"),
                });
                current_lines.clear();
            }
            current_heading = Some(line.trim().to_string());
            current_lines.push(line.to_string());
        } else if current_heading.is_some() {
            current_lines.push(line.to_string());
        }
    }

    if let Some(heading) = current_heading {
        entries.push(LedgerEntry {
            heading,
            text: current_lines.join("\n"),
        });
    }

    entries
}

struct ReleaseLedgerVerificationBase {
    ledger_path: String,
    dry_run_manifest_path: String,
    version: String,
    tag: String,
    created_unix_ms: u128,
}

struct ReleaseLedgerVerificationBuild {
    base: ReleaseLedgerVerificationBase,
    source_commit: String,
    gate_status: String,
    matched_entry_heading: String,
    required_markers: Vec<String>,
    status: ReleaseLedgerVerificationStatus,
    warnings: Vec<String>,
    violations: Vec<String>,
}

fn finalize_manifest(build: ReleaseLedgerVerificationBuild) -> ReleaseLedgerVerificationManifest {
    let ReleaseLedgerVerificationBuild {
        base,
        source_commit,
        gate_status,
        matched_entry_heading,
        required_markers,
        status,
        warnings,
        violations,
    } = build;
    ReleaseLedgerVerificationManifest {
        schema_version: 1,
        kind: "release_ledger_verification".to_string(),
        project: "CCL".to_string(),
        version: base.version,
        tag: base.tag,
        ledger_path: base.ledger_path,
        dry_run_manifest_path: base.dry_run_manifest_path,
        source_commit,
        gate_status,
        matched_entry_heading,
        required_markers,
        status,
        warnings,
        violations,
        created_unix_ms: base.created_unix_ms,
        github_ci_used_as_evidence: false,
    }
}

fn outcome_from_manifest(
    repo_root: &Path,
    manifest_path: PathBuf,
    manifest: ReleaseLedgerVerificationManifest,
) -> ReleaseLedgerVerificationOutcome {
    ReleaseLedgerVerificationOutcome {
        status: manifest.status.clone(),
        manifest_path: repo_relative_string(repo_root, manifest_path),
        manifest,
    }
}

fn write_manifest(
    path: &Path,
    manifest: &ReleaseLedgerVerificationManifest,
) -> Result<(), ReleaseLedgerVerificationError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn worst_status(
    current: ReleaseLedgerVerificationStatus,
    new_status: ReleaseLedgerVerificationStatus,
) -> ReleaseLedgerVerificationStatus {
    if release_status_rank(&new_status) > release_status_rank(&current) {
        new_status
    } else {
        current
    }
}

fn release_status_rank(status: &ReleaseLedgerVerificationStatus) -> u8 {
    match status {
        ReleaseLedgerVerificationStatus::Pass => 0,
        ReleaseLedgerVerificationStatus::PassWithWarnings => 1,
        ReleaseLedgerVerificationStatus::Fail => 2,
        ReleaseLedgerVerificationStatus::ContractFail => 3,
    }
}

fn generate_release_ledger_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("release-ledger-{}-{}", now, std::process::id())
}

fn system_time_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
}

fn resolve_input_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn normalize_path_string(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(path)
    }
}

fn repo_relative_string(repo: &Path, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let normalized_path = normalize_path_string(&path.to_string_lossy());
    let normalized_repo = repo
        .canonicalize()
        .map(|path| normalize_path_string(&path.to_string_lossy()))
        .unwrap_or_else(|_| normalize_path_string(&repo.to_string_lossy()));
    if let Ok(relative) = normalized_path.strip_prefix(&normalized_repo) {
        return relative.to_string_lossy().replace('\\', "/");
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_release_ledger_repo_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(repo: &Path, path: &str, content: &str) {
        let file = repo.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(file, content).unwrap();
    }

    fn dry_run_manifest(version: &str, tag: &str, source_commit: &str) -> ReleaseDryRunManifest {
        ReleaseDryRunManifest {
            schema_version: 1,
            kind: "release_dry_run".to_string(),
            project: "CCL".to_string(),
            version: version.to_string(),
            tag: tag.to_string(),
            repo: ".".to_string(),
            source: crate::release::ReleaseDryRunSource {
                head_commit: source_commit.to_string(),
                branch: "main".to_string(),
                tree_clean: true,
            },
            policy: crate::release::ReleaseDryRunPolicy {
                tag_created: false,
                artifacts_created: false,
                checksums_generated: false,
                github_release_created: false,
                crates_io_published: false,
                github_ci_used_as_evidence: false,
            },
            schema: crate::release::ReleaseDryRunSchemaSummary {
                release_manifest_schema_path: "schemas/ccl-release-manifest.schema.json"
                    .to_string(),
                schema_file_present: true,
                schema_json_valid: true,
            },
            gate: crate::release::ReleaseDryRunGateSummary {
                contract_path: "examples/ccl-admission-task-contract.json".to_string(),
                gate_status: "PASS".to_string(),
                gate_manifest_path: ".ccl/runs/gate-1/gate-run-manifest.json".to_string(),
            },
            ledger: crate::release::ReleaseDryRunLedgerSummary {
                release_entry_required_for_real_release: true,
                dry_run_entry_recorded: true,
            },
            warnings: vec![],
            violations: vec![],
            created_unix_ms: 1,
        }
    }

    fn write_dry_run_manifest(repo: &Path, manifest: &ReleaseDryRunManifest) -> PathBuf {
        let path = repo
            .join(".ccl")
            .join("runs")
            .join("release-dry-run-1")
            .join("release-dry-run-manifest.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, serde_json::to_vec_pretty(manifest).unwrap()).unwrap();
        path
    }

    fn base_ledger_text(
        version: &str,
        tag: &str,
        source_commit: &str,
        manifest_path: &str,
        status: &str,
    ) -> String {
        format!(
            r#"# CCL Project Ledger

## 2026-06-22 — Release Dry-Run v{version}

Status: {status}

### Scope

- Workstream: CCL Release Readiness
- Task type: release dry-run evidence
- Branch: feat/local-release-dry-run-seed
- PR: #40
- Base main HEAD: 804afe19255dc375fa4cd45d55215a7d7f92fe8c

### Release Dry-Run Proof

- Version: {version}
- Tag: {tag}
- Source commit: {source_commit}
- Release dry-run manifest: {manifest_path}
- Local CCL gate status: PASS
- GitHub CI used as evidence: NO
- Tag created: NO
- Release artifacts created: NO
- Checksums generated: NO
- GitHub Release created: NO
- crates.io publish: NO

### Boundary Conclusion

- local CCL evidence created: YES
- GitHub CI remains metadata: YES
"#
        )
    }

    fn run(
        repo: &Path,
        dry_run_manifest_path: &Path,
        ledger_path: &Path,
    ) -> ReleaseLedgerVerificationOutcome {
        run_release_ledger_verification(ReleaseLedgerVerificationRequest {
            repo: repo.to_path_buf(),
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_manifest_path.to_path_buf(),
            ledger_path: ledger_path.to_path_buf(),
            entry_heading: None,
        })
        .unwrap()
    }

    #[test]
    fn valid_release_dry_run_ledger_entry_passes() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.1.0",
                "abc123",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);
        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Pass);
        assert_eq!(outcome.manifest.source_commit, "abc123");
        assert_eq!(
            outcome.manifest.matched_entry_heading,
            "## 2026-06-22 — Release Dry-Run v0.1.0"
        );
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn missing_release_ledger_entry_fails() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(&repo, "ledger/project-ledger.md", "# CCL Project Ledger\n");

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_entry_missing"));
    }

    #[test]
    fn markers_from_different_entries_do_not_pass() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            r#"# CCL Project Ledger

## 2026-06-22 — Release Dry-Run v0.1.0

Status: PASS
- Version: 0.1.0
- Tag: v0.1.0

## 2026-06-22 — Release Dry-Run Evidence Fragment

- Source commit: abc123
- Release dry-run manifest: .ccl/runs/release-dry-run-1/release-dry-run-manifest.json
- Local CCL gate status: PASS
- GitHub CI used as evidence: NO
- Tag created: NO
- Release artifacts created: NO
- Checksums generated: NO
- GitHub Release created: NO
- crates.io publish: NO
"#,
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(
            outcome
                .manifest
                .violations
                .iter()
                .any(|violation| violation.starts_with("release_ledger_source_commit_mismatch"))
                || outcome
                    .manifest
                    .violations
                    .iter()
                    .any(|violation| violation == "release_ledger_entry_missing")
        );
    }

    #[test]
    fn version_mismatch_fails() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.2.0",
                "v0.2.0",
                "abc123",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_version_mismatch"));
    }

    #[test]
    fn tag_mismatch_fails() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.2.0",
                "abc123",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_tag_mismatch"));
    }

    #[test]
    fn source_commit_mismatch_fails() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.1.0",
                "def456",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_source_commit_mismatch"));
    }

    #[test]
    fn dry_run_manifest_path_mismatch_fails() {
        let repo = repo_dir();
        let manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.1.0",
                "abc123",
                ".ccl/runs/other/release-dry-run-manifest.json",
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_dry_run_manifest_mismatch"));
    }

    #[test]
    fn github_ci_used_as_evidence_yes_fails() {
        let repo = repo_dir();
        let mut manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        manifest.policy.github_ci_used_as_evidence = true;
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.1.0",
                "abc123",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_github_ci_evidence_violation"));
    }

    #[test]
    fn invalid_dry_run_manifest_is_contract_fail() {
        let repo = repo_dir();
        let dry_run_path = repo.join(".ccl/runs/release-dry-run-1/release-dry-run-manifest.json");
        if let Some(parent) = dry_run_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&dry_run_path, "{ not json }").unwrap();
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(&repo, "ledger/project-ledger.md", "# CCL Project Ledger\n");

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(
            outcome.status,
            ReleaseLedgerVerificationStatus::ContractFail
        );
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.contains("release_ledger_dry_run_manifest_missing")));
    }

    #[test]
    fn policy_side_effect_true_fails() {
        let repo = repo_dir();
        let mut manifest = dry_run_manifest("0.1.0", "v0.1.0", "abc123");
        manifest.policy.tag_created = true;
        let dry_run_path = write_dry_run_manifest(&repo, &manifest);
        let ledger_path = repo.join("ledger/project-ledger.md");
        write_file(
            &repo,
            "ledger/project-ledger.md",
            &base_ledger_text(
                "0.1.0",
                "v0.1.0",
                "abc123",
                &repo_relative_string(&repo, &dry_run_path),
                "PASS",
            ),
        );

        let outcome = run(&repo, &dry_run_path, &ledger_path);

        assert_eq!(outcome.status, ReleaseLedgerVerificationStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_ledger_tag_created_violation"));
    }
}
