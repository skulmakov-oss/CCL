use crate::release::ReleaseDryRunManifest;
use crate::release::{derive_release_tag, is_valid_release_tag, is_valid_release_version};
use crate::release_checksum::{
    ReleaseChecksumInput, ReleaseChecksumManifest, ReleaseChecksumStatus,
};
use crate::release_ledger::{ReleaseLedgerVerificationManifest, ReleaseLedgerVerificationStatus};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReleaseManifestDryAssemblyRequest {
    pub repo: PathBuf,
    pub version: String,
    pub dry_run_manifest_path: PathBuf,
    pub ledger_verification_manifest_path: PathBuf,
    pub checksum_manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseManifestDryAssemblyStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ReleaseManifestDryAssemblyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseManifestDryAssemblyStatus::Pass => write!(f, "PASS"),
            ReleaseManifestDryAssemblyStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ReleaseManifestDryAssemblyStatus::Fail => write!(f, "FAIL"),
            ReleaseManifestDryAssemblyStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

impl ReleaseManifestDryAssemblyStatus {
    pub fn is_pass_like(&self) -> bool {
        matches!(
            self,
            ReleaseManifestDryAssemblyStatus::Pass
                | ReleaseManifestDryAssemblyStatus::PassWithWarnings
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifestDrySource {
    pub commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifestDryEvidence {
    pub release_dry_run_manifest: String,
    pub release_ledger_verification_manifest: String,
    pub release_checksum_manifest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifestDryPolicy {
    pub dry_run_only: bool,
    pub tag_created: bool,
    pub release_artifacts_created: bool,
    pub github_release_created: bool,
    pub crates_io_published: bool,
    pub github_ci_used_as_evidence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifestDryManifest {
    pub schema_version: u32,
    pub kind: String,
    pub project: String,
    pub version: String,
    pub tag: String,
    pub source: ReleaseManifestDrySource,
    pub evidence: ReleaseManifestDryEvidence,
    pub checksums: Vec<ReleaseChecksumInput>,
    pub policy: ReleaseManifestDryPolicy,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub status: ReleaseManifestDryAssemblyStatus,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct ReleaseManifestDryAssemblyOutcome {
    pub status: ReleaseManifestDryAssemblyStatus,
    pub manifest: ReleaseManifestDryManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseManifestDryAssemblyError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("path error: {0}")]
    Path(String),
}

fn release_status_rank(status: &ReleaseManifestDryAssemblyStatus) -> u8 {
    match status {
        ReleaseManifestDryAssemblyStatus::Pass => 0,
        ReleaseManifestDryAssemblyStatus::PassWithWarnings => 1,
        ReleaseManifestDryAssemblyStatus::Fail => 2,
        ReleaseManifestDryAssemblyStatus::ContractFail => 3,
    }
}

fn worst_status(
    current: ReleaseManifestDryAssemblyStatus,
    new_status: ReleaseManifestDryAssemblyStatus,
) -> ReleaseManifestDryAssemblyStatus {
    if release_status_rank(&new_status) > release_status_rank(&current) {
        new_status
    } else {
        current
    }
}

pub fn run_release_manifest_dry_assemble(
    request: ReleaseManifestDryAssemblyRequest,
) -> Result<ReleaseManifestDryAssemblyOutcome, ReleaseManifestDryAssemblyError> {
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_id = generate_release_manifest_dry_assembly_id();
    let run_dir = repo_root.join(".ccl").join("runs").join(&run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("ccl-release-manifest.dry.json");
    let created_unix_ms = system_time_ms(SystemTime::now());

    let version = request.version.clone();
    let tag = derive_release_tag(&version);
    let dry_run_manifest_path = repo_relative_string(&repo_root, &request.dry_run_manifest_path);
    let ledger_verification_manifest_path =
        repo_relative_string(&repo_root, &request.ledger_verification_manifest_path);
    let checksum_manifest_path = repo_relative_string(&repo_root, &request.checksum_manifest_path);

    let mut warnings = Vec::new();
    let mut violations = Vec::new();
    let mut status = ReleaseManifestDryAssemblyStatus::Pass;
    let mut source_commit = "unknown".to_string();
    let mut checksums: Vec<ReleaseChecksumInput> = Vec::new();

    if !is_valid_release_version(&version) {
        violations.push("release_manifest_version_invalid".to_string());
        status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
    }

    if !is_valid_release_tag(&tag) {
        violations.push("release_manifest_tag_invalid".to_string());
        status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
    }

    let dry_run_manifest =
        match read_manifest::<ReleaseDryRunManifest>(&repo_root, &request.dry_run_manifest_path) {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                violations.push(format!(
                    "release_manifest_dry_run_manifest_invalid: {}",
                    error
                ));
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
                None
            }
        };

    let ledger_verification_manifest = match read_manifest::<ReleaseLedgerVerificationManifest>(
        &repo_root,
        &request.ledger_verification_manifest_path,
    ) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            violations.push(format!(
                "release_manifest_ledger_verification_manifest_invalid: {}",
                error
            ));
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
            None
        }
    };

    let checksum_manifest =
        match read_manifest::<ReleaseChecksumManifest>(&repo_root, &request.checksum_manifest_path)
        {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                violations.push(format!(
                    "release_manifest_checksum_manifest_invalid: {}",
                    error
                ));
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
                None
            }
        };

    if let (Some(dry_run_manifest), Some(ledger_manifest), Some(checksum_manifest)) = (
        dry_run_manifest.as_ref(),
        ledger_verification_manifest.as_ref(),
        checksum_manifest.as_ref(),
    ) {
        if dry_run_manifest.schema_version != 1 {
            violations.push("release_manifest_dry_run_schema_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if dry_run_manifest.kind != "release_dry_run" {
            violations.push("release_manifest_dry_run_kind_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if dry_run_manifest.project != "CCL" {
            violations.push("release_manifest_dry_run_project_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if dry_run_manifest.version != version {
            violations.push("release_manifest_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if dry_run_manifest.tag != tag {
            violations.push("release_manifest_tag_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if dry_run_manifest.source.head_commit.trim().is_empty() {
            violations.push("release_manifest_source_commit_missing".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        } else {
            source_commit = dry_run_manifest.source.head_commit.clone();
        }
        if !dry_run_manifest.source.tree_clean {
            violations.push("release_manifest_tree_not_clean".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if !dry_run_manifest.schema.schema_file_present {
            violations.push("release_manifest_schema_file_missing".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if !dry_run_manifest.schema.schema_json_valid {
            violations.push("release_manifest_schema_json_invalid".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if dry_run_manifest.policy.tag_created
            || dry_run_manifest.policy.artifacts_created
            || dry_run_manifest.policy.checksums_generated
            || dry_run_manifest.policy.github_release_created
            || dry_run_manifest.policy.crates_io_published
            || dry_run_manifest.policy.github_ci_used_as_evidence
        {
            violations.push("release_manifest_dry_run_side_effect_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        match dry_run_manifest.gate.gate_status.as_str() {
            "PASS" => {}
            "PASS WITH WARNINGS" => {
                warnings.push("release_manifest_dry_run_pass_with_warnings".to_string());
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::PassWithWarnings);
            }
            "FAIL" | "CONTRACT_FAIL" => {
                violations.push("release_manifest_dry_run_status_not_pass_like".to_string());
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
            }
            "" => {
                violations.push("release_manifest_dry_run_status_missing".to_string());
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
            }
            _ => {
                warnings.push("release_manifest_dry_run_unexpected_gate_status".to_string());
                status = worst_status(status, ReleaseManifestDryAssemblyStatus::PassWithWarnings);
            }
        }

        if ledger_manifest.schema_version != 1 {
            violations.push("release_manifest_ledger_schema_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if ledger_manifest.kind != "release_ledger_verification" {
            violations.push("release_manifest_ledger_kind_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if ledger_manifest.project != "CCL" {
            violations.push("release_manifest_ledger_project_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if ledger_manifest.version != version {
            violations.push("release_manifest_ledger_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if ledger_manifest.tag != tag {
            violations.push("release_manifest_ledger_tag_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if ledger_manifest.source_commit != source_commit {
            violations.push("release_manifest_ledger_source_commit_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if ledger_manifest.dry_run_manifest_path != dry_run_manifest_path {
            violations.push("release_manifest_ledger_dry_run_manifest_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if ledger_manifest.gate_status != "PASS" {
            violations.push("release_manifest_ledger_gate_status_not_pass".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if !ledger_manifest.github_ci_used_as_evidence {
            // expected boundary
        } else {
            violations.push("release_manifest_ledger_github_ci_evidence_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if ledger_manifest.status == ReleaseLedgerVerificationStatus::PassWithWarnings {
            warnings.push("release_manifest_ledger_verification_pass_with_warnings".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::PassWithWarnings);
        } else if ledger_manifest.status == ReleaseLedgerVerificationStatus::Fail
            || ledger_manifest.status == ReleaseLedgerVerificationStatus::ContractFail
        {
            violations.push("release_manifest_ledger_status_not_pass_like".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }

        if checksum_manifest.schema_version != 1 {
            violations.push("release_manifest_checksum_schema_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if checksum_manifest.kind != "release_checksum" {
            violations.push("release_manifest_checksum_kind_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if checksum_manifest.project != "CCL" {
            violations.push("release_manifest_checksum_project_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::ContractFail);
        }
        if checksum_manifest.version != version {
            violations.push("release_manifest_checksum_version_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.tag != tag {
            violations.push("release_manifest_checksum_tag_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.source_commit != source_commit {
            violations.push("release_manifest_checksum_source_commit_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.algorithm != "sha256" {
            violations.push("release_manifest_checksum_algorithm_mismatch".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.inputs.is_empty() {
            violations.push("release_manifest_checksum_inputs_missing".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        } else {
            checksums = checksum_manifest.inputs.clone();
        }
        if !checksum_manifest.policy.explicit_inputs_only {
            violations.push("release_manifest_checksum_explicit_inputs_only_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.policy.recursive_hashing {
            violations.push("release_manifest_checksum_recursive_hashing_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.policy.tag_created
            || checksum_manifest.policy.release_artifacts_created
            || checksum_manifest.policy.github_release_created
            || checksum_manifest.policy.crates_io_published
            || checksum_manifest.policy.github_ci_used_as_evidence
        {
            violations.push("release_manifest_checksum_side_effect_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if checksum_manifest.status == ReleaseChecksumStatus::PassWithWarnings {
            warnings.push("release_manifest_checksum_pass_with_warnings".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::PassWithWarnings);
        } else if checksum_manifest.status == ReleaseChecksumStatus::Fail
            || checksum_manifest.status == ReleaseChecksumStatus::ContractFail
        {
            violations.push("release_manifest_checksum_status_not_pass_like".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }

        if ledger_manifest.github_ci_used_as_evidence
            || dry_run_manifest.policy.github_ci_used_as_evidence
            || checksum_manifest.policy.github_ci_used_as_evidence
        {
            violations.push("release_manifest_github_ci_evidence_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if dry_run_manifest.version != ledger_manifest.version
            || dry_run_manifest.version != checksum_manifest.version
        {
            violations.push("release_manifest_version_consistency_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
        if dry_run_manifest.tag != ledger_manifest.tag
            || dry_run_manifest.tag != checksum_manifest.tag
        {
            violations.push("release_manifest_tag_consistency_violation".to_string());
            status = worst_status(status, ReleaseManifestDryAssemblyStatus::Fail);
        }
    }

    let manifest = ReleaseManifestDryManifest {
        schema_version: 1,
        kind: "ccl_release_manifest_dry".to_string(),
        project: "CCL".to_string(),
        version: version.clone(),
        tag: tag.clone(),
        source: ReleaseManifestDrySource {
            commit: source_commit,
        },
        evidence: ReleaseManifestDryEvidence {
            release_dry_run_manifest: dry_run_manifest_path,
            release_ledger_verification_manifest: ledger_verification_manifest_path,
            release_checksum_manifest: checksum_manifest_path,
        },
        checksums,
        policy: ReleaseManifestDryPolicy {
            dry_run_only: true,
            tag_created: false,
            release_artifacts_created: false,
            github_release_created: false,
            crates_io_published: false,
            github_ci_used_as_evidence: false,
        },
        warnings,
        violations,
        status: status.clone(),
        created_unix_ms,
    };

    if fs::create_dir_all(&run_dir).is_err() {
        // Keep the manifest in-memory if the directory cannot be created.
    } else if let Err(error) = write_manifest(&manifest_path, &manifest) {
        let mut manifest = manifest;
        manifest
            .violations
            .push(format!("release_manifest_write_failed: {}", error));
        manifest.status = worst_status(
            manifest.status,
            ReleaseManifestDryAssemblyStatus::ContractFail,
        );
        return Ok(ReleaseManifestDryAssemblyOutcome {
            status: manifest.status.clone(),
            manifest,
            manifest_path: repo_relative_string(&repo_root, manifest_path),
        });
    }

    Ok(ReleaseManifestDryAssemblyOutcome {
        status: manifest.status.clone(),
        manifest,
        manifest_path: repo_relative_string(&repo_root, manifest_path),
    })
}

fn read_manifest<T>(repo_root: &Path, path: &Path) -> Result<T, ReleaseManifestDryAssemblyError>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(resolve_input_path(repo_root, path)?)?;
    Ok(serde_json::from_str(&content)?)
}

fn resolve_input_path(
    repo_root: &Path,
    path: &Path,
) -> Result<PathBuf, ReleaseManifestDryAssemblyError> {
    let path = normalize_repo_relative_path(path)?;
    Ok(repo_root.join(path))
}

fn normalize_repo_relative_path(path: &Path) -> Result<PathBuf, ReleaseManifestDryAssemblyError> {
    if path.as_os_str().is_empty() {
        return Err(ReleaseManifestDryAssemblyError::Path(
            "release_manifest_input_empty".to_string(),
        ));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                return Err(ReleaseManifestDryAssemblyError::Path(
                    "release_manifest_input_path_traversal".to_string(),
                ))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ReleaseManifestDryAssemblyError::Path(
                    "release_manifest_absolute_path_rejected".to_string(),
                ))
            }
        }
    }

    if parts.is_empty() {
        return Err(ReleaseManifestDryAssemblyError::Path(
            "release_manifest_input_empty".to_string(),
        ));
    }

    Ok(PathBuf::from(parts.join("/")))
}

fn generate_release_manifest_dry_assembly_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("release-manifest-dry-{}-{}", now, std::process::id())
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

fn write_manifest(path: &Path, manifest: &ReleaseManifestDryManifest) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{GateRunManifest, GateStepManifest, GateStepName};
    use crate::verdict::{AdmissionStatus, VerdictStatus};
    use crate::{release, release_checksum, release_ledger};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_release_manifest_repo_{}_{}_{}",
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

    fn commit_files(repo: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
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
        }
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

    fn valid_repo() -> PathBuf {
        let repo = repo_dir();
        init_git_repo(&repo);
        commit_files(
            &repo,
            &[
                ("README.md", "# CCL\n"),
                ("docs/release-dry-run.md", "# Dry Run\n"),
                ("schemas/ccl-release-manifest.schema.json", "{}\n"),
            ],
        );
        repo
    }

    fn fake_gate_outcome(repo: &Path) -> crate::gate::GateRunOutcome {
        crate::gate::GateRunOutcome {
            manifest: GateRunManifest {
                schema_version: 1,
                gate_run_id: "gate-test".to_string(),
                contract_path: "examples/ccl-admission-task-contract.json".to_string(),
                contract_sha256: "abc".to_string(),
                repo_path: repo.to_string_lossy().into_owned(),
                status: AdmissionStatus::Pass,
                started_unix_ms: 0,
                finished_unix_ms: 0,
                github_ci_used_as_evidence: false,
                steps: vec![GateStepManifest {
                    name: GateStepName::Validation,
                    status: VerdictStatus::Pass.to_string(),
                    manifest_path: "validation-manifest.json".to_string(),
                }],
                warnings: Vec::new(),
                violations: Vec::new(),
            },
            manifest_path: "gate-run-manifest.json".to_string(),
        }
    }

    fn build_release_evidence() -> (
        PathBuf,
        ReleaseDryRunManifest,
        ReleaseLedgerVerificationManifest,
        ReleaseChecksumManifest,
        PathBuf,
        PathBuf,
        PathBuf,
    ) {
        let repo = valid_repo();
        let dry_run = release::run_release_dry_run_with_gate(
            release::ReleaseDryRunRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            },
            |_, _| Ok(fake_gate_outcome(&repo)),
        )
        .unwrap();

        let dry_run_manifest_path = dry_run.manifest_path.clone();
        let ledger_text = format!(
            r#"## 2026-06-22 — Release Dry-Run v0.1.0

Status: PASS
- Version: 0.1.0
- Tag: v0.1.0
- Source commit: {}
- Release dry-run manifest: {}
- Local CCL gate status: PASS
- GitHub CI used as evidence: NO
- Tag created: NO
- Release artifacts created: NO
- Checksums generated: NO
- GitHub Release created: NO
- crates.io publish: NO
"#,
            dry_run.manifest.source.head_commit, dry_run_manifest_path
        );
        fs::create_dir_all(repo.join("ledger")).unwrap();
        fs::write(repo.join("ledger/project-ledger.md"), ledger_text).unwrap();

        let ledger = release_ledger::run_release_ledger_verification(
            release_ledger::ReleaseLedgerVerificationRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                dry_run_manifest_path: PathBuf::from(&dry_run_manifest_path),
                ledger_path: PathBuf::from("ledger/project-ledger.md"),
                entry_heading: None,
            },
        )
        .unwrap();

        let checksum =
            release_checksum::run_release_checksum(release_checksum::ReleaseChecksumRequest {
                repo: repo.clone(),
                version: "0.1.0".to_string(),
                inputs: vec![
                    PathBuf::from("README.md"),
                    PathBuf::from("docs/release-dry-run.md"),
                ],
            });
        let ledger_manifest_path = ledger.manifest_path.clone();
        let checksum_manifest_path = checksum.manifest_path.clone();

        (
            repo,
            dry_run.manifest,
            ledger.manifest,
            checksum.manifest,
            PathBuf::from(dry_run_manifest_path),
            PathBuf::from(ledger_manifest_path),
            PathBuf::from(checksum_manifest_path),
        )
    }

    #[test]
    fn valid_dry_assembly_passes() {
        let (
            repo,
            dry_run_manifest,
            _ledger_manifest,
            _checksum_manifest,
            dry_run_path,
            ledger_path,
            checksum_path,
        ) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo: repo.clone(),
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert!(outcome.status.is_pass_like());
        assert_eq!(outcome.manifest.version, "0.1.0");
        assert_eq!(outcome.manifest.tag, "v0.1.0");
        assert_eq!(
            outcome.manifest.source.commit,
            dry_run_manifest.source.head_commit
        );
        assert_eq!(
            outcome.manifest.evidence.release_dry_run_manifest,
            dry_run_path.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            outcome
                .manifest
                .evidence
                .release_ledger_verification_manifest,
            ledger_path.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            outcome.manifest.evidence.release_checksum_manifest,
            checksum_path.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(outcome.manifest.checksums.len(), 2);
        assert!(outcome.manifest.policy.dry_run_only);
        assert!(!outcome.manifest.policy.tag_created);
        assert!(!outcome.manifest.policy.release_artifacts_created);
        assert!(!outcome.manifest.policy.github_release_created);
        assert!(!outcome.manifest.policy.crates_io_published);
        assert!(!outcome.manifest.policy.github_ci_used_as_evidence);
    }

    #[test]
    fn missing_dry_run_manifest_is_contract_fail() {
        let repo = valid_repo();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: PathBuf::from(".ccl/runs/missing/release-dry-run-manifest.json"),
            ledger_verification_manifest_path: PathBuf::from("ledger/project-ledger.md"),
            checksum_manifest_path: PathBuf::from("ledger/project-ledger.md"),
        })
        .unwrap();
        assert_eq!(
            outcome.status,
            ReleaseManifestDryAssemblyStatus::ContractFail
        );
    }

    #[test]
    fn missing_ledger_verification_manifest_is_contract_fail() {
        let (repo, _, _, _, dry_run_path, _, checksum_path) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: PathBuf::from(
                ".ccl/runs/missing/release-ledger-verification-manifest.json",
            ),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(
            outcome.status,
            ReleaseManifestDryAssemblyStatus::ContractFail
        );
    }

    #[test]
    fn missing_checksum_manifest_is_contract_fail() {
        let (repo, _, _, _, dry_run_path, ledger_path, _) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: PathBuf::from(
                ".ccl/runs/missing/release-checksum-manifest.json",
            ),
        })
        .unwrap();
        assert_eq!(
            outcome.status,
            ReleaseManifestDryAssemblyStatus::ContractFail
        );
    }

    #[test]
    fn invalid_json_is_contract_fail() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        fs::write(repo.join(&checksum_path), "not json").unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(
            outcome.status,
            ReleaseManifestDryAssemblyStatus::ContractFail
        );
    }

    #[test]
    fn wrong_manifest_kind_is_contract_fail() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&checksum_path)).unwrap()).unwrap();
        value["kind"] = serde_json::Value::String("wrong_kind".to_string());
        fs::write(
            repo.join(&checksum_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(
            outcome.status,
            ReleaseManifestDryAssemblyStatus::ContractFail
        );
    }

    #[test]
    fn version_mismatch_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.2.0".to_string(),
            dry_run_manifest_path: dry_run_path,
            ledger_verification_manifest_path: ledger_path,
            checksum_manifest_path: checksum_path,
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn tag_mismatch_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&checksum_path)).unwrap()).unwrap();
        value["tag"] = serde_json::Value::String("v0.2.0".to_string());
        fs::write(
            repo.join(&checksum_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn source_commit_mismatch_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&checksum_path)).unwrap()).unwrap();
        value["source_commit"] =
            serde_json::Value::String("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        fs::write(
            repo.join(&checksum_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn github_ci_used_as_evidence_true_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&ledger_path)).unwrap()).unwrap();
        value["github_ci_used_as_evidence"] = serde_json::Value::Bool(true);
        fs::write(
            repo.join(&ledger_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn release_side_effect_true_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&dry_run_path)).unwrap()).unwrap();
        value["policy"]["tag_created"] = serde_json::Value::Bool(true);
        fs::write(
            repo.join(&dry_run_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn recursive_hashing_true_fails() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(repo.join(&checksum_path)).unwrap()).unwrap();
        value["policy"]["recursive_hashing"] = serde_json::Value::Bool(true);
        fs::write(
            repo.join(&checksum_path),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.status, ReleaseManifestDryAssemblyStatus::Fail);
    }

    #[test]
    fn dry_manifest_records_checksum_entries() {
        let (repo, _, _, checksum_manifest, dry_run_path, ledger_path, checksum_path) =
            build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(outcome.manifest.checksums, checksum_manifest.inputs);
    }

    #[test]
    fn dry_manifest_records_evidence_paths() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path.clone(),
            ledger_verification_manifest_path: ledger_path.clone(),
            checksum_manifest_path: checksum_path.clone(),
        })
        .unwrap();
        assert_eq!(
            outcome.manifest.evidence.release_dry_run_manifest,
            dry_run_path.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            outcome
                .manifest
                .evidence
                .release_ledger_verification_manifest,
            ledger_path.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            outcome.manifest.evidence.release_checksum_manifest,
            checksum_path.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn dry_manifest_records_no_release_side_effects() {
        let (repo, _, _, _, dry_run_path, ledger_path, checksum_path) = build_release_evidence();
        let outcome = run_release_manifest_dry_assemble(ReleaseManifestDryAssemblyRequest {
            repo,
            version: "0.1.0".to_string(),
            dry_run_manifest_path: dry_run_path,
            ledger_verification_manifest_path: ledger_path,
            checksum_manifest_path: checksum_path,
        })
        .unwrap();
        assert!(outcome.manifest.policy.dry_run_only);
        assert!(!outcome.manifest.policy.tag_created);
        assert!(!outcome.manifest.policy.release_artifacts_created);
        assert!(!outcome.manifest.policy.github_release_created);
        assert!(!outcome.manifest.policy.crates_io_published);
        assert!(!outcome.manifest.policy.github_ci_used_as_evidence);
    }
}
