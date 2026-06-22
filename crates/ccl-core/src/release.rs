use crate::gate::{self, GateRunOutcome, GateRunRequest};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReleaseDryRunRequest {
    pub repo: PathBuf,
    pub version: String,
    pub contract_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseDryRunStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ReleaseDryRunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseDryRunStatus::Pass => write!(f, "PASS"),
            ReleaseDryRunStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ReleaseDryRunStatus::Fail => write!(f, "FAIL"),
            ReleaseDryRunStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunSource {
    pub head_commit: String,
    pub branch: String,
    pub tree_clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunPolicy {
    pub tag_created: bool,
    pub artifacts_created: bool,
    pub checksums_generated: bool,
    pub github_release_created: bool,
    pub crates_io_published: bool,
    pub github_ci_used_as_evidence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunSchemaSummary {
    pub release_manifest_schema_path: String,
    pub schema_file_present: bool,
    pub schema_json_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunGateSummary {
    pub contract_path: String,
    pub gate_status: String,
    pub gate_manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunLedgerSummary {
    pub release_entry_required_for_real_release: bool,
    pub dry_run_entry_recorded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDryRunManifest {
    pub schema_version: u32,
    pub kind: String,
    pub project: String,
    pub version: String,
    pub tag: String,
    pub repo: String,
    pub source: ReleaseDryRunSource,
    pub policy: ReleaseDryRunPolicy,
    pub schema: ReleaseDryRunSchemaSummary,
    pub gate: ReleaseDryRunGateSummary,
    pub ledger: ReleaseDryRunLedgerSummary,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct ReleaseDryRunOutcome {
    pub status: ReleaseDryRunStatus,
    pub manifest: ReleaseDryRunManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseDryRunError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn run_release_dry_run(
    request: ReleaseDryRunRequest,
) -> Result<ReleaseDryRunOutcome, ReleaseDryRunError> {
    run_release_dry_run_with_gate(request, |contract_path, repo| {
        gate::run_gate(GateRunRequest {
            contract_path: contract_path.to_path_buf(),
            repo: repo.to_path_buf(),
        })
        .map_err(|error| error.to_string())
    })
}

pub fn run_release_dry_run_with_gate<F>(
    request: ReleaseDryRunRequest,
    mut gate_runner: F,
) -> Result<ReleaseDryRunOutcome, ReleaseDryRunError>
where
    F: FnMut(&Path, &Path) -> Result<GateRunOutcome, String>,
{
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_id = generate_release_dry_run_id();
    let run_dir = repo_root.join(".ccl").join("runs").join(&run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("release-dry-run-manifest.json");
    let created_unix_ms = system_time_ms(SystemTime::now());

    let repo_path = request.repo.to_string_lossy().into_owned();
    let version = request.version.clone();
    let tag = derive_release_tag(&version);

    let mut warnings = Vec::new();
    let mut violations = Vec::new();
    let mut status = ReleaseDryRunStatus::Pass;

    if !is_valid_release_version(&version) {
        violations.push("invalid_version: version must match MAJOR.MINOR.PATCH".to_string());
        status = worst_status(status, ReleaseDryRunStatus::ContractFail);
    }

    if !is_valid_release_tag(&tag) {
        violations.push("invalid_tag: tag must match vMAJOR.MINOR.PATCH".to_string());
        status = worst_status(status, ReleaseDryRunStatus::ContractFail);
    }

    let source = inspect_repo_state(&repo_root, &mut violations, &mut status);
    let schema = inspect_schema(&repo_root, &mut violations, &mut status);

    let gate_summary = match gate_runner(&request.contract_path, &request.repo) {
        Ok(outcome) => {
            let gate_status = outcome.manifest.status.to_string();
            match outcome.manifest.status {
                crate::verdict::AdmissionStatus::Pass => {}
                crate::verdict::AdmissionStatus::PassWithWarnings => {
                    warnings.push("gate_pass_with_warnings".to_string());
                    status = worst_status(status, ReleaseDryRunStatus::PassWithWarnings);
                }
                crate::verdict::AdmissionStatus::Fail => {
                    violations.push("gate_status_fail".to_string());
                    status = worst_status(status, ReleaseDryRunStatus::Fail);
                }
                crate::verdict::AdmissionStatus::ContractFail => {
                    violations.push("gate_status_contract_fail".to_string());
                    status = worst_status(status, ReleaseDryRunStatus::ContractFail);
                }
                crate::verdict::AdmissionStatus::InternalError => {
                    violations.push("gate_status_internal_error".to_string());
                    status = worst_status(status, ReleaseDryRunStatus::Fail);
                }
            }

            ReleaseDryRunGateSummary {
                contract_path: request.contract_path.to_string_lossy().into_owned(),
                gate_status,
                gate_manifest_path: outcome.manifest_path,
            }
        }
        Err(reason) => {
            violations.push(format!("gate_error: {}", reason));
            status = worst_status(status, ReleaseDryRunStatus::Fail);
            ReleaseDryRunGateSummary {
                contract_path: request.contract_path.to_string_lossy().into_owned(),
                gate_status: ReleaseDryRunStatus::Fail.to_string(),
                gate_manifest_path: "N/A".to_string(),
            }
        }
    };

    if source.tree_clean
        && schema.schema_file_present
        && schema.schema_json_valid
        && matches!(status, ReleaseDryRunStatus::Pass)
        && !warnings.is_empty()
    {
        status = ReleaseDryRunStatus::PassWithWarnings;
    }

    let manifest = ReleaseDryRunManifest {
        schema_version: 1,
        kind: "release_dry_run".to_string(),
        project: "CCL".to_string(),
        version,
        tag,
        repo: repo_path,
        source,
        policy: ReleaseDryRunPolicy {
            tag_created: false,
            artifacts_created: false,
            checksums_generated: false,
            github_release_created: false,
            crates_io_published: false,
            github_ci_used_as_evidence: false,
        },
        schema,
        gate: gate_summary,
        ledger: ReleaseDryRunLedgerSummary {
            release_entry_required_for_real_release: true,
            dry_run_entry_recorded: true,
        },
        warnings,
        violations,
        created_unix_ms,
    };

    write_manifest(&manifest_path, &manifest)?;
    Ok(ReleaseDryRunOutcome {
        status,
        manifest,
        manifest_path: repo_relative_string(&repo_root, &manifest_path),
    })
}

fn inspect_repo_state(
    repo_root: &Path,
    violations: &mut Vec<String>,
    status: &mut ReleaseDryRunStatus,
) -> ReleaseDryRunSource {
    let head_commit = run_git_output(repo_root, &["rev-parse", "HEAD"]).unwrap_or_else(|reason| {
        violations.push(format!("git_head_failed: {}", reason));
        *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
        "unknown".to_string()
    });

    let branch = run_git_output(repo_root, &["branch", "--show-current"])
        .unwrap_or_else(|_| String::new())
        .trim()
        .to_string();
    let branch = if branch.is_empty() {
        "detached".to_string()
    } else {
        branch
    };

    let status_output = run_git_output(repo_root, &["status", "--short", "--untracked-files=all"])
        .unwrap_or_else(|reason| {
            violations.push(format!("git_status_failed: {}", reason));
            *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
            String::new()
        });

    let tree_clean = status_output.trim().is_empty();
    if !tree_clean {
        violations.push("dirty_tree_detected".to_string());
        *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
    }

    ReleaseDryRunSource {
        head_commit: head_commit.trim().to_string(),
        branch,
        tree_clean,
    }
}

fn inspect_schema(
    repo_root: &Path,
    violations: &mut Vec<String>,
    status: &mut ReleaseDryRunStatus,
) -> ReleaseDryRunSchemaSummary {
    let schema_path = repo_root
        .join("schemas")
        .join("ccl-release-manifest.schema.json");
    let release_manifest_schema_path = "schemas/ccl-release-manifest.schema.json".to_string();
    let schema_file_present = schema_path.exists();
    let mut schema_json_valid = false;

    if schema_file_present {
        match fs::read_to_string(&schema_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(_) => {
                    schema_json_valid = true;
                }
                Err(error) => {
                    violations.push(format!("schema_json_invalid: {}", error));
                    *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
                }
            },
            Err(error) => {
                violations.push(format!("schema_read_failed: {}", error));
                *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
            }
        }
    } else {
        violations.push("schema_file_missing".to_string());
        *status = worst_status(status.clone(), ReleaseDryRunStatus::Fail);
    }

    ReleaseDryRunSchemaSummary {
        release_manifest_schema_path,
        schema_file_present,
        schema_json_valid,
    }
}

pub fn is_valid_release_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    if parts.next().is_some() {
        return false;
    }
    [major, minor, patch].into_iter().all(|component| {
        matches!(
            component,
            Some(value)
                if !value.is_empty()
                    && value.chars().all(|ch| ch.is_ascii_digit())
                    && (value == "0" || !value.starts_with('0'))
        )
    })
}

pub fn derive_release_tag(version: &str) -> String {
    format!("v{}", version)
}

pub fn is_valid_release_tag(tag: &str) -> bool {
    tag.strip_prefix('v')
        .map(is_valid_release_version)
        .unwrap_or(false)
}

fn worst_status(
    current: ReleaseDryRunStatus,
    new_status: ReleaseDryRunStatus,
) -> ReleaseDryRunStatus {
    if release_status_rank(&new_status) > release_status_rank(&current) {
        new_status
    } else {
        current
    }
}

fn release_status_rank(status: &ReleaseDryRunStatus) -> u8 {
    match status {
        ReleaseDryRunStatus::Pass => 0,
        ReleaseDryRunStatus::PassWithWarnings => 1,
        ReleaseDryRunStatus::Fail => 2,
        ReleaseDryRunStatus::ContractFail => 3,
    }
}

fn run_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = stderr.trim().to_string();
        if !message.is_empty() {
            return Err(message);
        }
        let message = stdout.trim().to_string();
        if !message.is_empty() {
            return Err(message);
        }
        return Err(format!("git {:?} failed", args));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn generate_release_dry_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("release-dry-run-{}-{}", now, std::process::id())
}

fn system_time_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
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

fn write_manifest(path: &Path, manifest: &ReleaseDryRunManifest) -> Result<(), ReleaseDryRunError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{
        GateRunManifest, GateRunStatus, GateStepManifest, GateStepName, GateViolation, GateWarning,
    };
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_release_repo_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn init_git_repo(repo: &Path) {
        let status = Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "init"])
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn write_commit_file(repo: &Path, path: &str, content: &str) {
        let file = repo.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file, content).unwrap();
        let status = Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", path])
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "-c",
                "user.name=CCL",
                "-c",
                "user.email=ccl@example.com",
                "commit",
                "-m",
                "initial",
                "--quiet",
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn helper_gate_outcome(
        repo: &Path,
        status: GateRunStatus,
        warnings: Vec<GateWarning>,
        violations: Vec<GateViolation>,
    ) -> GateRunOutcome {
        let admission_status = status.to_string();
        let manifest = GateRunManifest {
            schema_version: 1,
            gate_run_id: "gate-test".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            repo_path: repo.to_string_lossy().into_owned(),
            status,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            github_ci_used_as_evidence: false,
            steps: vec![
                GateStepManifest {
                    name: GateStepName::Validation,
                    status: "PASS".to_string(),
                    manifest_path: ".ccl/runs/validation-test/validation-run-manifest.json"
                        .to_string(),
                },
                GateStepManifest {
                    name: GateStepName::Scope,
                    status: "PASS".to_string(),
                    manifest_path: ".ccl/runs/scope-test/scope-check-manifest.json".to_string(),
                },
                GateStepManifest {
                    name: GateStepName::Admission,
                    status: admission_status,
                    manifest_path: ".ccl/runs/admission-test/admission-verdict.json".to_string(),
                },
            ],
            warnings,
            violations,
        };

        GateRunOutcome {
            manifest,
            manifest_path: ".ccl/runs/gate-test/gate-run-manifest.json".to_string(),
        }
    }

    fn valid_schema_text() -> String {
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://github.com/skulmakov-oss/CCL/schemas/ccl-release-manifest.schema.json",
  "title": "CCL Release Manifest",
  "type": "object"
}"#
        .to_string()
    }

    fn prepare_repo_with_schema(schema_text: &str) -> PathBuf {
        let repo = repo_dir();
        init_git_repo(&repo);
        write_commit_file(&repo, ".gitignore", ".ccl/runs/\n");
        let schema_path = repo.join("schemas/ccl-release-manifest.schema.json");
        if let Some(parent) = schema_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&schema_path, schema_text).unwrap();
        let status = Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", ".gitignore", "schemas"])
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "-c",
                "user.name=CCL",
                "-c",
                "user.email=ccl@example.com",
                "commit",
                "-m",
                "initial",
                "--quiet",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        repo
    }

    #[test]
    fn valid_version_derives_expected_tag() {
        assert!(is_valid_release_version("0.1.0"));
        assert_eq!(derive_release_tag("0.1.0"), "v0.1.0");
        assert!(is_valid_release_tag("v0.1.0"));
    }

    #[test]
    fn invalid_version_is_rejected() {
        assert!(!is_valid_release_version("v0.1.0"));
        assert!(!is_valid_release_version("0.1"));
        assert!(!is_valid_release_tag("0.1.0"));
    }

    #[test]
    fn dry_run_manifest_marks_no_release_side_effects() {
        let repo = prepare_repo_with_schema(&valid_schema_text());
        let outcome = run_release_dry_run_with_gate(
            ReleaseDryRunRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            },
            |_contract, repo| {
                Ok(helper_gate_outcome(
                    repo,
                    GateRunStatus::Pass,
                    vec![],
                    vec![],
                ))
            },
        )
        .unwrap();

        assert_eq!(outcome.status, ReleaseDryRunStatus::Pass);
        assert!(!outcome.manifest.policy.tag_created);
        assert!(!outcome.manifest.policy.artifacts_created);
        assert!(!outcome.manifest.policy.checksums_generated);
        assert!(!outcome.manifest.policy.github_release_created);
        assert!(!outcome.manifest.policy.crates_io_published);
        assert!(!outcome.manifest.policy.github_ci_used_as_evidence);
        assert!(
            outcome
                .manifest
                .ledger
                .release_entry_required_for_real_release
        );
        assert!(outcome.manifest.ledger.dry_run_entry_recorded);
        assert!(outcome.manifest.schema.schema_file_present);
        assert!(outcome.manifest.schema.schema_json_valid);
        assert!(outcome.manifest.source.tree_clean);
        assert_eq!(outcome.manifest.tag, "v0.1.0");
        assert!(repo.join(&outcome.manifest_path).exists());
    }

    #[test]
    fn schema_json_parse_failure_is_violation() {
        let repo = prepare_repo_with_schema("{ not json }");
        let outcome = run_release_dry_run_with_gate(
            ReleaseDryRunRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            },
            |_contract, repo| {
                Ok(helper_gate_outcome(
                    repo,
                    GateRunStatus::Pass,
                    vec![],
                    vec![],
                ))
            },
        )
        .unwrap();

        assert_eq!(outcome.status, ReleaseDryRunStatus::Fail);
        assert!(!outcome.manifest.schema.schema_json_valid);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.contains("schema_json_invalid")));
    }

    #[test]
    fn dirty_tree_is_rejected_or_reported() {
        let repo = prepare_repo_with_schema(&valid_schema_text());
        fs::write(repo.join("dirty.txt"), "dirty").unwrap();
        let outcome = run_release_dry_run_with_gate(
            ReleaseDryRunRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            },
            |_contract, repo| {
                Ok(helper_gate_outcome(
                    repo,
                    GateRunStatus::Pass,
                    vec![],
                    vec![],
                ))
            },
        )
        .unwrap();

        assert_eq!(outcome.status, ReleaseDryRunStatus::Fail);
        assert!(!outcome.manifest.source.tree_clean);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.contains("dirty_tree_detected")));
    }

    #[test]
    fn gate_warnings_turn_dry_run_into_warnings() {
        let repo = prepare_repo_with_schema(&valid_schema_text());
        let outcome = run_release_dry_run_with_gate(
            ReleaseDryRunRequest {
                repo,
                version: "0.1.0".to_string(),
                contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            },
            |_contract, repo| {
                Ok(helper_gate_outcome(
                    repo,
                    GateRunStatus::PassWithWarnings,
                    vec![GateWarning {
                        kind: "demo_warning".to_string(),
                        reason: "gate warned".to_string(),
                    }],
                    vec![],
                ))
            },
        )
        .unwrap();

        assert_eq!(outcome.status, ReleaseDryRunStatus::PassWithWarnings);
        assert!(outcome
            .manifest
            .warnings
            .iter()
            .any(|warning| warning.contains("gate_pass_with_warnings")));
    }
}
