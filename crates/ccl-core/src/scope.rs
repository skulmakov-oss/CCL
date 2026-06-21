use crate::task_contract::TaskContract;
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeCheckStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ScopeCheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeCheckStatus::Pass => write!(f, "PASS"),
            ScopeCheckStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ScopeCheckStatus::Fail => write!(f, "FAIL"),
            ScopeCheckStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeChangeOperation {
    Create,
    Edit,
    Delete,
    Rename,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeDecision {
    Pass,
    Fail,
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeLimitStatus {
    WithinLimits,
    Exceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeFileEntry {
    pub path: String,
    pub operation: ScopeChangeOperation,
    pub tracked: bool,
    pub status_source: String,
    pub allowed: bool,
    pub forbidden: bool,
    pub decision: ScopeDecision,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeViolation {
    pub kind: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeWarning {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCheckSummary {
    pub changed_files_count: usize,
    pub tracked_changed_files_count: usize,
    pub untracked_files_count: usize,
    pub diff_added_lines: usize,
    pub diff_deleted_lines: usize,
    pub diff_total_lines: usize,
    pub max_changed_files: usize,
    pub max_diff_lines: usize,
    pub limit_status: ScopeLimitStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCheckManifest {
    pub schema_version: u32,
    pub scope_run_id: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub repo_path: String,
    pub base_ref: String,
    pub status: ScopeCheckStatus,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
    pub github_ci_used_as_evidence: bool,
    pub summary: ScopeCheckSummary,
    #[serde(default)]
    pub files: Vec<ScopeFileEntry>,
    #[serde(default)]
    pub violations: Vec<ScopeViolation>,
    #[serde(default)]
    pub warnings: Vec<ScopeWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScopeCheckOutcome {
    pub manifest: ScopeCheckManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ScopeCheckError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("git command failed: {0}")]
    Git(String),
}

#[derive(Debug, Clone)]
struct RawStatusEntry {
    path: String,
    tracked: bool,
    status_code: String,
}

#[derive(Debug, Clone)]
struct RawDiffEntry {
    path: String,
    operation: ScopeChangeOperation,
}

pub fn run_scope_check(
    contract_path: &Path,
    repo: &Path,
) -> Result<ScopeCheckOutcome, ScopeCheckError> {
    let scope_run_id = generate_scope_run_id();
    let repo_root = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let run_dir = repo_root.join(".ccl").join("runs").join(&scope_run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("scope-check-manifest.json");
    let started_unix_ms = system_time_ms(SystemTime::now());

    let repo_path_string = repo.to_string_lossy().into_owned();
    let contract_path_string = contract_path.to_string_lossy().into_owned();

    let contract_bytes = match fs::read(contract_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let manifest = build_contract_fail_manifest(
                scope_run_id,
                contract_path_string,
                String::new(),
                repo_path_string,
                started_unix_ms,
                Some(format!("contract_read_failed: {}", error)),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(ScopeCheckOutcome {
                manifest,
                manifest_path: repo_relative_string(&repo_root, &manifest_path),
            });
        }
    };

    let contract_sha256 = sha256_hex(&contract_bytes);
    let contract = match serde_json::from_slice::<TaskContract>(&contract_bytes) {
        Ok(contract) => contract,
        Err(error) => {
            let manifest = build_contract_fail_manifest(
                scope_run_id,
                contract_path_string,
                contract_sha256,
                repo_path_string,
                started_unix_ms,
                Some(format!("contract_parse_failed: {}", error)),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(ScopeCheckOutcome {
                manifest,
                manifest_path: repo_relative_string(&repo_root, &manifest_path),
            });
        }
    };

    let contract_report = contract.validate();
    if contract_report.status.is_failure() {
        let reason = contract_report
            .errors
            .first()
            .map(|error| format!("contract_validation_failed: {}", error.0))
            .or_else(|| Some("contract_validation_failed".to_string()));
        let manifest = build_contract_fail_manifest(
            scope_run_id,
            contract_path_string,
            contract_sha256,
            repo_path_string,
            started_unix_ms,
            reason,
        );
        write_manifest(&manifest_path, &manifest)?;
        return Ok(ScopeCheckOutcome {
            manifest,
            manifest_path: repo_relative_string(&repo_root, &manifest_path),
        });
    }

    let mut warnings = contract_report
        .warnings
        .into_iter()
        .map(|warning| ScopeWarning {
            kind: "contract_warning".to_string(),
            reason: warning.0,
        })
        .collect::<Vec<_>>();

    let base_ref = "HEAD".to_string();
    let status_output = run_git_string(
        &repo_root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let diff_name_status = run_git_lines(&repo_root, &["diff", "--name-status", "HEAD"])?;
    let numstat_lines = run_git_lines(&repo_root, &["diff", "--numstat", "HEAD"])?;
    let ls_untracked = run_git_lines(&repo_root, &["ls-files", "--others", "--exclude-standard"])?;
    let _base_sha = run_git_string(&repo_root, &["rev-parse", "HEAD"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut diff_map = BTreeMap::new();
    for record in parse_diff_name_status(&diff_name_status) {
        diff_map.insert(record.path.clone(), record.operation.clone());
    }

    let mut entries_by_path: BTreeMap<String, ScopeFileEntry> = BTreeMap::new();
    let mut tracked_changed_files_count = 0usize;
    let mut untracked_files_count = 0usize;

    for record in parse_status_records(&status_output)? {
        if record.tracked {
            let operation = diff_map
                .get(&record.path)
                .cloned()
                .unwrap_or_else(|| classify_operation(&record.status_code));
            let (allowed, forbidden, decision, reason, warnings_here) =
                evaluate_path(&contract, &record.path, true);
            warnings.extend(warnings_here);
            entries_by_path.insert(
                record.path.clone(),
                ScopeFileEntry {
                    path: record.path,
                    operation,
                    tracked: true,
                    status_source: "git_status".to_string(),
                    allowed,
                    forbidden,
                    decision,
                    reason,
                },
            );
            tracked_changed_files_count += 1;
        } else {
            let (allowed, forbidden, decision, reason, warnings_here) =
                evaluate_path(&contract, &record.path, false);
            warnings.extend(warnings_here);
            entries_by_path.insert(
                record.path.clone(),
                ScopeFileEntry {
                    path: record.path,
                    operation: ScopeChangeOperation::Create,
                    tracked: false,
                    status_source: "git_status".to_string(),
                    allowed,
                    forbidden,
                    decision,
                    reason,
                },
            );
            untracked_files_count += 1;
        }
    }

    let mut observed_paths = BTreeSet::new();
    for path in entries_by_path.keys() {
        observed_paths.insert(path.clone());
    }

    for path in ls_untracked {
        let normalized = normalize_repo_relative_path(&path);
        if observed_paths.contains(&normalized) {
            continue;
        }
        let (allowed, forbidden, decision, reason, warnings_here) =
            evaluate_path(&contract, &normalized, false);
        warnings.extend(warnings_here);
        entries_by_path.insert(
            normalized.clone(),
            ScopeFileEntry {
                path: normalized.clone(),
                operation: ScopeChangeOperation::Create,
                tracked: false,
                status_source: "git_ls_files".to_string(),
                allowed,
                forbidden,
                decision,
                reason,
            },
        );
        observed_paths.insert(normalized);
        untracked_files_count += 1;
    }

    let changed_files_count = entries_by_path.len();
    let mut diff_added_lines = 0usize;
    let mut diff_deleted_lines = 0usize;

    for line in numstat_lines {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let added = parts.next().unwrap_or_default();
        let deleted = parts.next().unwrap_or_default();
        diff_added_lines += parse_numstat_value(added);
        diff_deleted_lines += parse_numstat_value(deleted);
    }

    for path in entries_by_path.keys() {
        if !entries_by_path
            .get(path)
            .map(|entry| entry.tracked)
            .unwrap_or(false)
        {
            diff_added_lines += count_lines_in_file(&repo_root.join(path)).unwrap_or(0);
        }
    }

    let diff_total_lines = diff_added_lines.saturating_add(diff_deleted_lines);
    let max_changed_files = contract.scope_limits.max_changed_files;
    let max_diff_lines = contract.scope_limits.max_diff_lines;

    let mut violations = Vec::new();
    let mut limit_status = ScopeLimitStatus::WithinLimits;

    if changed_files_count > max_changed_files {
        limit_status = ScopeLimitStatus::Exceeded;
        violations.push(ScopeViolation {
            kind: "diff_limit_exceeded".to_string(),
            path: String::new(),
            reason: format!(
                "changed files {} exceeded max_changed_files {}",
                changed_files_count, max_changed_files
            ),
        });
    }

    if diff_total_lines > max_diff_lines {
        limit_status = ScopeLimitStatus::Exceeded;
        violations.push(ScopeViolation {
            kind: "diff_limit_exceeded".to_string(),
            path: String::new(),
            reason: format!(
                "diff lines {} exceeded max_diff_lines {}",
                diff_total_lines, max_diff_lines
            ),
        });
    }

    for entry in entries_by_path.values() {
        if !entry.reason.is_empty() && entry.reason.contains("path traversal") {
            violations.push(ScopeViolation {
                kind: "path_traversal_detected".to_string(),
                path: entry.path.clone(),
                reason: entry.reason.clone(),
            });
        }
        if entry.forbidden {
            violations.push(ScopeViolation {
                kind: "forbidden_path_changed".to_string(),
                path: entry.path.clone(),
                reason: entry.reason.clone(),
            });
        } else if !entry.allowed && !contract.allowed_paths.is_empty() {
            violations.push(ScopeViolation {
                kind: "outside_allowed_scope".to_string(),
                path: entry.path.clone(),
                reason: entry.reason.clone(),
            });
        }
        if matches!(entry.operation, ScopeChangeOperation::Unknown) {
            violations.push(ScopeViolation {
                kind: "unknown_file_operation".to_string(),
                path: entry.path.clone(),
                reason: entry.reason.clone(),
            });
        }
    }

    let status = if !violations.is_empty() {
        ScopeCheckStatus::Fail
    } else if !warnings.is_empty() {
        ScopeCheckStatus::PassWithWarnings
    } else {
        ScopeCheckStatus::Pass
    };

    let summary = ScopeCheckSummary {
        changed_files_count,
        tracked_changed_files_count,
        untracked_files_count,
        diff_added_lines,
        diff_deleted_lines,
        diff_total_lines,
        max_changed_files,
        max_diff_lines,
        limit_status,
    };

    let manifest = ScopeCheckManifest {
        schema_version: 1,
        scope_run_id,
        contract_path: contract_path_string,
        contract_sha256,
        repo_path: repo_path_string,
        base_ref,
        status,
        started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        summary,
        files: entries_by_path.into_values().collect(),
        violations,
        warnings,
        reason: None,
    };

    write_manifest(&manifest_path, &manifest)?;

    Ok(ScopeCheckOutcome {
        manifest,
        manifest_path: repo_relative_string(&repo_root, &manifest_path),
    })
}

fn build_contract_fail_manifest(
    scope_run_id: String,
    contract_path: String,
    contract_sha256: String,
    repo_path: String,
    started_unix_ms: u128,
    reason: Option<String>,
) -> ScopeCheckManifest {
    ScopeCheckManifest {
        schema_version: 1,
        scope_run_id,
        contract_path,
        contract_sha256,
        repo_path,
        base_ref: "HEAD".to_string(),
        status: ScopeCheckStatus::ContractFail,
        started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        summary: ScopeCheckSummary {
            changed_files_count: 0,
            tracked_changed_files_count: 0,
            untracked_files_count: 0,
            diff_added_lines: 0,
            diff_deleted_lines: 0,
            diff_total_lines: 0,
            max_changed_files: 25,
            max_diff_lines: 1500,
            limit_status: ScopeLimitStatus::WithinLimits,
        },
        files: vec![],
        violations: vec![],
        warnings: vec![],
        reason,
    }
}

fn evaluate_path(
    contract: &TaskContract,
    path: &str,
    tracked: bool,
) -> (bool, bool, ScopeDecision, String, Vec<ScopeWarning>) {
    let mut warnings = Vec::new();
    let normalized = normalize_repo_relative_path(path);
    if has_path_traversal(&normalized) {
        return (
            false,
            false,
            ScopeDecision::Fail,
            "path traversal detected".to_string(),
            warnings,
        );
    }

    let forbidden_pattern = match_pattern(&contract.forbidden_paths, &normalized);
    if let Some(pattern) = forbidden_pattern {
        return (
            false,
            true,
            ScopeDecision::Fail,
            format!("matched forbidden path {}", pattern),
            warnings,
        );
    }

    let allowed_pattern = match_pattern(&contract.allowed_paths, &normalized);
    if let Some(pattern) = allowed_pattern {
        return (
            true,
            false,
            ScopeDecision::Pass,
            format!("matched allowed path {}", pattern),
            warnings,
        );
    }

    if contract.allowed_paths.is_empty() {
        warnings.push(ScopeWarning {
            kind: "allowed_paths_missing".to_string(),
            reason: "allowed_paths empty; compatibility warning".to_string(),
        });
        return (
            true,
            false,
            ScopeDecision::Warn,
            if tracked {
                "allowed_paths empty; compatibility warning".to_string()
            } else {
                "untracked file with empty allowed_paths".to_string()
            },
            warnings,
        );
    }

    (
        false,
        false,
        ScopeDecision::Fail,
        "did not match allowed paths".to_string(),
        warnings,
    )
}

fn parse_status_records(output: &str) -> Result<Vec<RawStatusEntry>, ScopeCheckError> {
    let mut records = Vec::new();
    let mut items = output
        .as_bytes()
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
        .map(|item| String::from_utf8_lossy(item).into_owned())
        .peekable();

    while let Some(item) = items.next() {
        if item.len() < 3 {
            return Err(ScopeCheckError::Git(format!(
                "invalid git status record: {}",
                item
            )));
        }
        let status_code = item[0..2].to_string();
        let path = normalize_repo_relative_path(item[3..].trim());
        if status_code == "??" {
            records.push(RawStatusEntry {
                path,
                tracked: false,
                status_code,
            });
            continue;
        }

        if status_code.contains('R') || status_code.contains('C') {
            let new_path = items.next().ok_or_else(|| {
                ScopeCheckError::Git("rename status record missing new path".to_string())
            })?;
            records.push(RawStatusEntry {
                path: normalize_repo_relative_path(&new_path),
                tracked: true,
                status_code,
            });
            continue;
        }

        records.push(RawStatusEntry {
            path,
            tracked: true,
            status_code,
        });
    }

    Ok(records)
}

fn parse_diff_name_status(output: &[String]) -> Vec<RawDiffEntry> {
    let mut records = Vec::new();
    for line in output {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or_default();
        match status.chars().next().unwrap_or(' ') {
            'R' | 'C' => {
                let _old = parts.next();
                if let Some(new_path) = parts.next().or_else(|| parts.next()) {
                    records.push(RawDiffEntry {
                        path: normalize_repo_relative_path(new_path),
                        operation: ScopeChangeOperation::Rename,
                    });
                }
            }
            'A' => {
                if let Some(path) = parts.next() {
                    records.push(RawDiffEntry {
                        path: normalize_repo_relative_path(path),
                        operation: ScopeChangeOperation::Create,
                    });
                }
            }
            'D' => {
                if let Some(path) = parts.next() {
                    records.push(RawDiffEntry {
                        path: normalize_repo_relative_path(path),
                        operation: ScopeChangeOperation::Delete,
                    });
                }
            }
            'M' => {
                if let Some(path) = parts.next() {
                    records.push(RawDiffEntry {
                        path: normalize_repo_relative_path(path),
                        operation: ScopeChangeOperation::Edit,
                    });
                }
            }
            _ => {
                if let Some(path) = parts.next() {
                    records.push(RawDiffEntry {
                        path: normalize_repo_relative_path(path),
                        operation: ScopeChangeOperation::Unknown,
                    });
                }
            }
        }
    }
    records
}

fn parse_numstat_value(value: &str) -> usize {
    match value.trim() {
        "-" => 0,
        other => other.parse::<usize>().unwrap_or(0),
    }
}

fn count_lines_in_file(path: &Path) -> Result<usize, std::io::Error> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        return Ok(0);
    }
    let mut lines = bytes.iter().filter(|byte| **byte == b'\n').count();
    if !bytes.ends_with(b"\n") {
        lines += 1;
    }
    Ok(lines)
}

fn normalize_repo_relative_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }
    normalized
}

fn has_path_traversal(path: &str) -> bool {
    let candidate = Path::new(path);
    candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    })
}

fn match_pattern(patterns: &[String], path: &str) -> Option<String> {
    let normalized_path = normalize_repo_relative_path(path);
    patterns.iter().find_map(|pattern| {
        let normalized_pattern = normalize_repo_relative_path(pattern);
        if normalized_pattern.ends_with("/**") {
            let prefix = normalized_pattern.trim_end_matches("/**");
            if normalized_path == prefix || normalized_path.starts_with(&format!("{}/", prefix)) {
                Some(pattern.clone())
            } else {
                None
            }
        } else if normalized_path == normalized_pattern {
            Some(pattern.clone())
        } else {
            None
        }
    })
}

fn run_git_string(repo: &Path, args: &[&str]) -> Result<String, ScopeCheckError> {
    let output = Command::new("git").args(args).current_dir(repo).output()?;
    if !output.status.success() {
        return Err(ScopeCheckError::Git(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_git_lines(repo: &Path, args: &[&str]) -> Result<Vec<String>, ScopeCheckError> {
    let output = run_git_string(repo, args)?;
    Ok(output.lines().map(|line| line.to_string()).collect())
}

fn write_manifest(path: &Path, manifest: &ScopeCheckManifest) -> Result<(), ScopeCheckError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn generate_scope_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("scope-{}-{}", now, std::process::id())
}

fn system_time_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
}

fn repo_relative_string(repo: &Path, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let normalized_path = normalize_repo_relative_path(&path.to_string_lossy());
    let normalized_repo = repo
        .canonicalize()
        .map(|path| normalize_repo_relative_path(&path.to_string_lossy()))
        .unwrap_or_else(|_| normalize_repo_relative_path(&repo.to_string_lossy()));
    if let Ok(relative) = Path::new(&normalized_path).strip_prefix(&normalized_repo) {
        return relative.to_string_lossy().replace('\\', "/");
    }
    normalized_path
}

fn classify_operation(status_code: &str) -> ScopeChangeOperation {
    if status_code == "??" {
        ScopeChangeOperation::Create
    } else if status_code.contains('R') || status_code.contains('C') {
        ScopeChangeOperation::Rename
    } else if status_code.contains('D') {
        ScopeChangeOperation::Delete
    } else if status_code.contains('A') {
        ScopeChangeOperation::Create
    } else if status_code.contains('M') {
        ScopeChangeOperation::Edit
    } else {
        ScopeChangeOperation::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_scope_repo_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        let status = Command::new("git").arg("init").arg(&dir).status().unwrap();
        assert!(status.success());
        let _ = Command::new("git")
            .args([
                "-C",
                dir.to_str().unwrap(),
                "config",
                "user.email",
                "ccl@example.com",
            ])
            .status()
            .unwrap();
        let _ = Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "config", "user.name", "CCL"])
            .status()
            .unwrap();
        dir
    }

    fn contract_root() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_scope_contracts_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_and_commit(repo: &Path, path: &str, content: &str) {
        let file_path = repo.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
        let _ = Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", "."])
            .status()
            .unwrap();
        let _ = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "commit",
                "-m",
                "init",
                "--quiet",
            ])
            .status()
            .unwrap();
    }

    fn contract_file(content: &str) -> PathBuf {
        let path = contract_root().join("contract.json");
        fs::write(&path, content).unwrap();
        path
    }

    fn run_scope(repo: &Path, contract: &Path) -> ScopeCheckOutcome {
        run_scope_check(contract, repo).unwrap()
    }

    fn base_contract(
        allowed_paths: &[&str],
        forbidden_paths: &[&str],
        limits: (usize, usize),
    ) -> String {
        let allowed = allowed_paths
            .iter()
            .map(|path| format!(r#""{}""#, path))
            .collect::<Vec<_>>()
            .join(", ");
        let forbidden = forbidden_paths
            .iter()
            .map(|path| format!(r#""{}""#, path))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"{{
  "project": "CCL",
  "workstream": "Scope",
  "task_type": "guard_gate",
  "objective": "Scope check",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": [{}],
  "forbidden_paths": [{}],
  "required_validation": ["scope check"],
  "scope_limits": {{
    "max_changed_files": {},
    "max_diff_lines": {}
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
            allowed, forbidden, limits.0, limits.1
        )
    }

    #[test]
    fn empty_working_tree_scope_check_passes() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Pass);
        assert_eq!(outcome.manifest.summary.changed_files_count, 0);
        assert_eq!(outcome.manifest.files.len(), 0);
        assert!(Path::new(&repo.join(outcome.manifest_path)).exists());
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn allowed_path_passes() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::write(repo.join("src/app.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Pass);
        assert_eq!(outcome.manifest.files.len(), 1);
        assert_eq!(outcome.manifest.files[0].decision, ScopeDecision::Pass);
        assert!(outcome.manifest.files[0].path.contains('/'));
    }

    #[test]
    fn forbidden_path_fails() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::create_dir_all(repo.join(".github/workflows")).unwrap();
        fs::write(repo.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
        let contract = contract_file(&base_contract(
            &["src/**", ".github/**"],
            &[".github/**"],
            (25, 1500),
        ));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "forbidden_path_changed"));
    }

    #[test]
    fn forbidden_path_wins_over_allowed() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/allowed.rs"), "pub fn f() {}\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &["src/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(outcome.manifest.files.iter().any(|entry| entry.forbidden));
    }

    #[test]
    fn outside_allowed_scope_fails() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::write(repo.join("notes.txt"), "hello\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "outside_allowed_scope"));
    }

    #[test]
    fn untracked_file_is_detected() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let new_path = repo.join("src/new.rs");
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&new_path, "pub fn n() {}\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.summary.untracked_files_count, 1);
        assert!(outcome.manifest.files.iter().any(|entry| !entry.tracked));
    }

    #[test]
    fn untracked_file_outside_allowed_scope_fails() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let debug_path = repo.join("tmp/debug.log");
        if let Some(parent) = debug_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&debug_path, "debug\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "outside_allowed_scope"));
    }

    #[test]
    fn max_changed_files_exceeded_fails() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        for i in 0..4 {
            fs::write(repo.join(format!("src/file{}.rs", i)), "pub fn f() {}\n").unwrap();
        }
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (2, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(matches!(
            outcome.manifest.summary.limit_status,
            ScopeLimitStatus::Exceeded
        ));
    }

    #[test]
    fn max_diff_lines_exceeded_fails() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("line {}\n", i));
        }
        fs::write(repo.join("src/app.rs"), &content).unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 10)));

        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::Fail);
        assert!(outcome.manifest.summary.diff_total_lines > 10);
    }

    #[test]
    fn missing_contract_writes_contract_fail_manifest() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let missing = repo.join("missing.json");
        let outcome = run_scope_check(&missing, &repo).unwrap();
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .starts_with("contract_read_failed: "));
    }

    #[test]
    fn invalid_contract_writes_contract_fail_manifest() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        let contract = contract_file("{ not json");
        let outcome = run_scope(&repo, &contract);
        assert_eq!(outcome.manifest.status, ScopeCheckStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("contract_parse_failed"));
    }

    #[test]
    fn scope_manifest_is_created_and_ci_flag_false() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::write(repo.join("src/app.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        assert!(Path::new(&repo.join(outcome.manifest_path)).exists());
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn path_normalization_uses_repo_relative_slashes() {
        let repo = repo_dir();
        write_and_commit(&repo, "src/app.rs", "fn main() {}\n");
        fs::write(repo.join("src/app.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        let contract = contract_file(&base_contract(&["src/**"], &[".github/**"], (25, 1500)));

        let outcome = run_scope(&repo, &contract);
        let path = &outcome.manifest.files[0].path;
        assert!(!path.contains('\\'));
        assert!(path.contains('/'));
    }
}
