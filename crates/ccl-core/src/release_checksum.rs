use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReleaseChecksumRequest {
    pub repo: PathBuf,
    pub version: String,
    pub inputs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseChecksumStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ReleaseChecksumStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseChecksumStatus::Pass => write!(f, "PASS"),
            ReleaseChecksumStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ReleaseChecksumStatus::Fail => write!(f, "FAIL"),
            ReleaseChecksumStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

impl ReleaseChecksumStatus {
    pub fn is_pass_like(&self) -> bool {
        matches!(
            self,
            ReleaseChecksumStatus::Pass | ReleaseChecksumStatus::PassWithWarnings
        )
    }

    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            ReleaseChecksumStatus::Fail | ReleaseChecksumStatus::ContractFail
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseChecksumInput {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseChecksumPolicy {
    pub explicit_inputs_only: bool,
    pub recursive_hashing: bool,
    pub tag_created: bool,
    pub release_artifacts_created: bool,
    pub github_release_created: bool,
    pub crates_io_published: bool,
    pub github_ci_used_as_evidence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseChecksumManifest {
    pub schema_version: u32,
    pub kind: String,
    pub project: String,
    pub version: String,
    pub tag: String,
    pub repo: String,
    pub source_commit: String,
    pub algorithm: String,
    pub inputs: Vec<ReleaseChecksumInput>,
    pub policy: ReleaseChecksumPolicy,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub status: ReleaseChecksumStatus,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct ReleaseChecksumOutcome {
    pub status: ReleaseChecksumStatus,
    pub manifest: ReleaseChecksumManifest,
    pub manifest_path: String,
}

#[derive(Debug, Clone)]
struct PreparedInput {
    manifest_path: String,
    size_bytes: u64,
    sha256: String,
}

fn release_status_rank(status: &ReleaseChecksumStatus) -> u8 {
    match status {
        ReleaseChecksumStatus::Pass => 0,
        ReleaseChecksumStatus::PassWithWarnings => 1,
        ReleaseChecksumStatus::Fail => 2,
        ReleaseChecksumStatus::ContractFail => 3,
    }
}

fn worst_status(
    current: ReleaseChecksumStatus,
    new_status: ReleaseChecksumStatus,
) -> ReleaseChecksumStatus {
    if release_status_rank(&new_status) > release_status_rank(&current) {
        new_status
    } else {
        current
    }
}

pub fn run_release_checksum(request: ReleaseChecksumRequest) -> ReleaseChecksumOutcome {
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_id = generate_release_checksum_id();
    let run_dir = repo_root.join(".ccl").join("runs").join(&run_id);
    let manifest_path = run_dir.join("release-checksum-manifest.json");
    let created_unix_ms = system_time_ms(SystemTime::now());
    let version = request.version.clone();
    let tag = derive_release_tag(&version);

    let warnings = Vec::new();
    let mut violations = Vec::new();
    let mut status = ReleaseChecksumStatus::Pass;
    let mut source_commit = "unknown".to_string();
    let mut can_hash_inputs = true;

    if !is_valid_release_version(&version) {
        violations.push("release_checksum_version_invalid".to_string());
        status = worst_status(status, ReleaseChecksumStatus::ContractFail);
        can_hash_inputs = false;
    }

    if !is_valid_release_tag(&tag) {
        violations.push("release_checksum_tag_invalid".to_string());
        status = worst_status(status, ReleaseChecksumStatus::ContractFail);
    }

    if request.inputs.is_empty() {
        violations.push("release_checksum_no_inputs".to_string());
        status = worst_status(status, ReleaseChecksumStatus::ContractFail);
        can_hash_inputs = false;
    }

    match run_git_output(&repo_root, &["rev-parse", "HEAD"]) {
        Ok(commit) if !commit.trim().is_empty() => {
            source_commit = commit.trim().to_string();
        }
        Ok(_) => {
            violations.push("release_checksum_source_commit_missing".to_string());
            status = worst_status(status, ReleaseChecksumStatus::Fail);
        }
        Err(reason) => {
            violations.push(format!(
                "release_checksum_source_commit_unavailable: {}",
                reason
            ));
            status = worst_status(status, ReleaseChecksumStatus::Fail);
        }
    }

    let mut prepared_inputs = Vec::new();
    for input in &request.inputs {
        match prepare_input(&repo_root, input) {
            Ok(prepared) => prepared_inputs.push(prepared),
            Err(violation) => {
                violations.push(violation);
                status = worst_status(status, ReleaseChecksumStatus::ContractFail);
                can_hash_inputs = false;
            }
        }
    }

    let mut manifest_inputs = Vec::new();
    if can_hash_inputs {
        for prepared in prepared_inputs {
            manifest_inputs.push(ReleaseChecksumInput {
                path: prepared.manifest_path,
                size_bytes: prepared.size_bytes,
                sha256: prepared.sha256,
            });
        }
    }

    if can_hash_inputs
        && manifest_inputs.len() != request.inputs.len()
        && !request.inputs.is_empty()
    {
        violations.push("release_checksum_input_preparation_mismatch".to_string());
        status = worst_status(status, ReleaseChecksumStatus::ContractFail);
        manifest_inputs.clear();
    }

    let mut manifest = ReleaseChecksumManifest {
        schema_version: 1,
        kind: "release_checksum".to_string(),
        project: "CCL".to_string(),
        version: version.clone(),
        tag: tag.clone(),
        repo: request.repo.to_string_lossy().into_owned(),
        source_commit,
        algorithm: "sha256".to_string(),
        inputs: manifest_inputs,
        policy: ReleaseChecksumPolicy {
            explicit_inputs_only: true,
            recursive_hashing: false,
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
        manifest
            .violations
            .push("release_checksum_run_dir_creation_failed".to_string());
        manifest.status = worst_status(manifest.status, ReleaseChecksumStatus::ContractFail);
    } else if let Err(error) = write_manifest(&manifest_path, &manifest) {
        manifest
            .violations
            .push(format!("release_checksum_manifest_write_failed: {}", error));
        manifest.status = worst_status(manifest.status, ReleaseChecksumStatus::ContractFail);
    }

    ReleaseChecksumOutcome {
        status: manifest.status.clone(),
        manifest,
        manifest_path: repo_relative_string(&repo_root, manifest_path),
    }
}

fn prepare_input(repo_root: &Path, input: &Path) -> Result<PreparedInput, String> {
    let normalized = normalize_repo_relative_input(input)?;
    if is_forbidden_repo_input(&normalized) {
        return Err(format!("release_checksum_forbidden_input: {}", normalized));
    }

    let candidate = repo_root.join(&normalized);
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        format!(
            "release_checksum_input_missing_or_unreadable: {} ({})",
            normalized, error
        )
    })?;

    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(format!(
            "release_checksum_symlink_input_rejected: {}",
            normalized
        ));
    }
    if !file_type.is_file() {
        return Err(format!(
            "release_checksum_directory_input_rejected: {}",
            normalized
        ));
    }

    let canonical_repo = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let canonical_candidate = candidate.canonicalize().map_err(|error| {
        format!(
            "release_checksum_input_canonicalization_failed: {} ({})",
            normalized, error
        )
    })?;

    if !canonical_candidate.starts_with(&canonical_repo) {
        return Err(format!(
            "release_checksum_input_escapes_repo: {}",
            normalized
        ));
    }

    let bytes = fs::read(&canonical_candidate).map_err(|error| {
        format!(
            "release_checksum_input_read_failed: {} ({})",
            normalized, error
        )
    })?;
    let size_bytes = bytes.len() as u64;
    let sha256 = sha256_hex(&bytes);

    Ok(PreparedInput {
        manifest_path: normalized,
        size_bytes,
        sha256,
    })
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
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

fn normalize_repo_relative_input(input: &Path) -> Result<String, String> {
    if input.as_os_str().is_empty() {
        return Err("release_checksum_input_empty".to_string());
    }

    let mut parts = Vec::new();
    for component in input.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                return Err("release_checksum_input_path_traversal".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("release_checksum_absolute_path_rejected".to_string());
            }
        }
    }

    if parts.is_empty() {
        return Err("release_checksum_input_empty".to_string());
    }

    Ok(parts.join("/"))
}

fn is_forbidden_repo_input(path: &str) -> bool {
    let mut parts = path.split('/');
    match parts.next() {
        Some(".git") => true,
        Some(".ccl") => matches!(parts.next(), Some("runs")),
        _ => false,
    }
}

fn generate_release_checksum_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("release-checksum-{}-{}", now, std::process::id())
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

fn write_manifest(path: &Path, manifest: &ReleaseChecksumManifest) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_release_checksum_repo_{}_{}_{}",
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

    fn valid_repo(files: &[(&str, &str)]) -> PathBuf {
        let repo = repo_dir();
        init_git_repo(&repo);
        commit_files(&repo, files);
        repo
    }

    fn checksum_request(repo: &Path, version: &str, inputs: Vec<&str>) -> ReleaseChecksumRequest {
        ReleaseChecksumRequest {
            repo: repo.to_path_buf(),
            version: version.to_string(),
            inputs: inputs.into_iter().map(PathBuf::from).collect(),
        }
    }

    fn expected_empty_sha256() -> String {
        sha256_hex(b"")
    }

    #[test]
    fn valid_single_file_checksum_passes() {
        let repo = valid_repo(&[("README.md", "hello world\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["README.md"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::Pass);
        assert_eq!(outcome.manifest.version, "0.1.0");
        assert_eq!(outcome.manifest.tag, "v0.1.0");
        assert!(!outcome.manifest.source_commit.is_empty());
        assert_eq!(outcome.manifest.algorithm, "sha256");
        assert_eq!(outcome.manifest.inputs.len(), 1);
        assert_eq!(outcome.manifest.inputs[0].path, "README.md");
        assert_eq!(
            outcome.manifest.inputs[0].size_bytes,
            "hello world\n".len() as u64
        );
        assert_eq!(
            outcome.manifest.inputs[0].sha256,
            sha256_hex("hello world\n".as_bytes())
        );
        assert!(!outcome.manifest.policy.tag_created);
        assert!(!outcome.manifest.policy.release_artifacts_created);
        assert!(!outcome.manifest.policy.github_release_created);
        assert!(!outcome.manifest.policy.crates_io_published);
        assert!(!outcome.manifest.policy.github_ci_used_as_evidence);
        assert!(repo.join(&outcome.manifest_path).exists());
    }

    #[test]
    fn valid_multiple_file_checksum_passes() {
        let repo = valid_repo(&[
            ("README.md", "hello\n"),
            ("docs/release-dry-run.md", "dry\n"),
        ]);
        let outcome = run_release_checksum(checksum_request(
            &repo,
            "0.1.0",
            vec!["README.md", "docs/release-dry-run.md"],
        ));

        assert_eq!(outcome.status, ReleaseChecksumStatus::Pass);
        assert_eq!(outcome.manifest.inputs.len(), 2);
        assert_eq!(outcome.manifest.inputs[0].path, "README.md");
        assert_eq!(outcome.manifest.inputs[1].path, "docs/release-dry-run.md");
    }

    #[test]
    fn empty_file_checksum_is_stable() {
        let repo = valid_repo(&[("empty.txt", "")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["empty.txt"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::Pass);
        assert_eq!(outcome.manifest.inputs[0].size_bytes, 0);
        assert_eq!(outcome.manifest.inputs[0].sha256, expected_empty_sha256());
    }

    #[test]
    fn no_inputs_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec![]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_checksum_no_inputs"));
    }

    #[test]
    fn invalid_version_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "v0.1.0", vec!["README.md"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_checksum_version_invalid"));
    }

    #[test]
    fn missing_input_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["missing.txt"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.contains("release_checksum_input_missing_or_unreadable")));
    }

    #[test]
    fn directory_input_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let dir = repo.join("docs");
        fs::create_dir_all(&dir).unwrap();
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["docs"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_checksum_directory_input_rejected: docs"));
    }

    #[test]
    fn path_traversal_input_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome =
            run_release_checksum(checksum_request(&repo, "0.1.0", vec!["../outside.txt"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_checksum_input_path_traversal"));
    }

    #[test]
    fn absolute_path_outside_repo_is_contract_fail() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outside = std::env::temp_dir().join(format!(
            "ccl_checksum_outside_{}_{}.txt",
            std::process::id(),
            system_time_ms(SystemTime::now())
        ));
        fs::write(&outside, "outside").unwrap();
        let outcome = run_release_checksum(checksum_request(
            &repo,
            "0.1.0",
            vec![outside.to_str().unwrap()],
        ));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation == "release_checksum_absolute_path_rejected"));
    }

    #[test]
    fn input_under_ccl_runs_is_rejected() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let path = ".ccl/runs/release-checksum-1/manifest.json";
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec![path]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::ContractFail);
        assert!(outcome.manifest.violations.iter().any(|violation| violation
            == "release_checksum_forbidden_input: .ccl/runs/release-checksum-1/manifest.json"));
    }

    #[test]
    fn duplicate_input_behavior_is_deterministic() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome = run_release_checksum(checksum_request(
            &repo,
            "0.1.0",
            vec!["README.md", "README.md"],
        ));

        assert_eq!(outcome.status, ReleaseChecksumStatus::Pass);
        assert_eq!(outcome.manifest.inputs.len(), 2);
        assert_eq!(outcome.manifest.inputs[0].path, "README.md");
        assert_eq!(outcome.manifest.inputs[1].path, "README.md");
        assert_eq!(
            outcome.manifest.inputs[0].sha256,
            outcome.manifest.inputs[1].sha256
        );
    }

    #[test]
    fn manifest_records_size_and_sha256() {
        let repo = valid_repo(&[("README.md", "checksum bytes\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["README.md"]));

        assert_eq!(outcome.status, ReleaseChecksumStatus::Pass);
        assert_eq!(
            outcome.manifest.inputs[0].size_bytes,
            "checksum bytes\n".len() as u64
        );
        assert_eq!(
            outcome.manifest.inputs[0].sha256,
            sha256_hex("checksum bytes\n".as_bytes())
        );
    }

    #[test]
    fn manifest_records_no_release_side_effects() {
        let repo = valid_repo(&[("README.md", "hello\n")]);
        let outcome = run_release_checksum(checksum_request(&repo, "0.1.0", vec!["README.md"]));

        assert!(!outcome.manifest.policy.tag_created);
        assert!(!outcome.manifest.policy.release_artifacts_created);
        assert!(!outcome.manifest.policy.github_release_created);
        assert!(!outcome.manifest.policy.crates_io_published);
        assert!(!outcome.manifest.policy.github_ci_used_as_evidence);
        assert!(outcome.manifest.policy.explicit_inputs_only);
        assert!(!outcome.manifest.policy.recursive_hashing);
    }
}
