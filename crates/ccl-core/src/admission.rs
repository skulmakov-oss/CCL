use crate::ledger::{self, LedgerVerificationStatus};
use crate::scope::{ScopeCheckManifest, ScopeCheckStatus};
use crate::task_contract::TaskContract;
use crate::validation_runner::{ValidationRunManifest, ValidationRunStatus};
use crate::verdict::AdmissionStatus;
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct AdmissionVerdictRequest {
    pub contract_path: PathBuf,
    pub repo: PathBuf,
    pub validation_manifest_path: PathBuf,
    pub scope_manifest_path: PathBuf,
    pub ledger_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionViolation {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionWarning {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionEvidenceSummary {
    pub validation_status: String,
    pub scope_status: String,
    pub validation_github_ci_used_as_evidence: bool,
    pub scope_github_ci_used_as_evidence: bool,
    pub scope_violations_count: usize,
    pub validation_commands_count: usize,
    pub required_validation_failures_count: usize,
    pub missing_command_result_artifacts_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger_verification_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger_verification_manifest_path: Option<String>,
    pub ledger_exists: bool,
    pub ledger_update_required: bool,
    pub contract_sha256_matches_validation: bool,
    pub contract_sha256_matches_scope: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionVerdictManifest {
    pub schema_version: u32,
    pub admission_run_id: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub repo_path: String,
    pub validation_manifest_path: String,
    pub scope_manifest_path: String,
    pub ledger_path: String,
    pub status: AdmissionStatus,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
    pub github_ci_used_as_evidence: bool,
    pub evidence: AdmissionEvidenceSummary,
    #[serde(default)]
    pub violations: Vec<AdmissionViolation>,
    #[serde(default)]
    pub warnings: Vec<AdmissionWarning>,
    pub decision_rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdmissionVerdictOutcome {
    pub manifest: AdmissionVerdictManifest,
    pub manifest_path: String,
}

struct AdmissionManifestBase {
    admission_run_id: String,
    contract_path: String,
    contract_sha256: String,
    repo_path: String,
    validation_manifest_path: String,
    scope_manifest_path: String,
    ledger_path: String,
    started_unix_ms: u128,
}

#[derive(Debug, thiserror::Error)]
pub enum AdmissionVerdictError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ledger verification error: {0}")]
    Ledger(String),
}

pub fn run_admission_verdict(
    request: AdmissionVerdictRequest,
) -> Result<AdmissionVerdictOutcome, AdmissionVerdictError> {
    let admission_run_id = generate_admission_run_id();
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_dir = repo_root.join(".ccl").join("runs").join(&admission_run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("admission-verdict.json");
    let started_unix_ms = system_time_ms(SystemTime::now());

    let contract_path_string = request.contract_path.to_string_lossy().into_owned();
    let repo_path_string = request.repo.to_string_lossy().into_owned();
    let validation_manifest_path_string = request
        .validation_manifest_path
        .to_string_lossy()
        .into_owned();
    let scope_manifest_path_string = request.scope_manifest_path.to_string_lossy().into_owned();
    let ledger_path_string = request.ledger_path.to_string_lossy().into_owned();

    let contract_path = resolve_input_path(&repo_root, &request.contract_path);
    let contract_bytes = match fs::read(&contract_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let manifest = build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256: String::new(),
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some(format!("contract_read_failed: {}", error)),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    let contract_sha256 = sha256_hex(&contract_bytes);
    let contract = match serde_json::from_slice::<TaskContract>(&contract_bytes) {
        Ok(contract) => contract,
        Err(error) => {
            let manifest = build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some(format!("contract_parse_failed: {}", error)),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
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
            AdmissionManifestBase {
                admission_run_id,
                contract_path: contract_path_string,
                contract_sha256,
                repo_path: repo_path_string,
                validation_manifest_path: validation_manifest_path_string,
                scope_manifest_path: scope_manifest_path_string,
                ledger_path: ledger_path_string,
                started_unix_ms,
            },
            reason,
        );
        write_manifest(&manifest_path, &manifest)?;
        return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
    }

    let mut warnings = contract_report
        .warnings
        .into_iter()
        .map(|warning| AdmissionWarning {
            kind: "contract_warning".to_string(),
            reason: warning.0,
        })
        .collect::<Vec<_>>();

    let validation_manifest_path =
        resolve_input_path(&repo_root, &request.validation_manifest_path);
    let validation_manifest = match load_validation_manifest(&validation_manifest_path) {
        Ok(manifest) => manifest,
        Err(reason) => {
            let manifest = build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some(reason),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    let scope_manifest_path = resolve_input_path(&repo_root, &request.scope_manifest_path);
    let scope_manifest = match load_scope_manifest(&scope_manifest_path) {
        Ok(manifest) => manifest,
        Err(reason) => {
            let manifest = build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some(reason),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    let mut violations = Vec::new();

    let validation_contract_sha_matches = validation_manifest.contract_sha256 == contract_sha256;
    if !validation_contract_sha_matches {
        return write_contract_fail_manifest(
            &repo_root,
            &manifest_path,
            build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some("validation_manifest_contract_sha256_mismatch".to_string()),
            ),
        );
    }

    let scope_contract_sha_matches = scope_manifest.contract_sha256 == contract_sha256;
    if !scope_contract_sha_matches {
        return write_contract_fail_manifest(
            &repo_root,
            &manifest_path,
            build_contract_fail_manifest(
                AdmissionManifestBase {
                    admission_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    validation_manifest_path: validation_manifest_path_string,
                    scope_manifest_path: scope_manifest_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some("scope_manifest_contract_sha256_mismatch".to_string()),
            ),
        );
    }

    if validation_manifest.github_ci_used_as_evidence {
        violations.push(AdmissionViolation {
            kind: "validation_github_ci_used_as_evidence".to_string(),
            reason: "validation manifest marked GitHub CI as evidence".to_string(),
        });
    }

    if scope_manifest.github_ci_used_as_evidence {
        violations.push(AdmissionViolation {
            kind: "scope_github_ci_used_as_evidence".to_string(),
            reason: "scope manifest marked GitHub CI as evidence".to_string(),
        });
    }

    if !matches!(
        validation_manifest.status,
        ValidationRunStatus::Pass | ValidationRunStatus::PassWithWarnings
    ) {
        if validation_manifest.status == ValidationRunStatus::ContractFail {
            return write_contract_fail_manifest(
                &repo_root,
                &manifest_path,
                build_contract_fail_manifest(
                    AdmissionManifestBase {
                        admission_run_id,
                        contract_path: contract_path_string,
                        contract_sha256,
                        repo_path: repo_path_string,
                        validation_manifest_path: validation_manifest_path_string,
                        scope_manifest_path: scope_manifest_path_string,
                        ledger_path: ledger_path_string,
                        started_unix_ms,
                    },
                    Some("validation_manifest_contract_fail".to_string()),
                ),
            );
        }
        violations.push(AdmissionViolation {
            kind: "validation_status_fail".to_string(),
            reason: format!("validation status {}", validation_manifest.status),
        });
    } else if validation_manifest.status == ValidationRunStatus::PassWithWarnings {
        warnings.push(AdmissionWarning {
            kind: "validation_status_pass_with_warnings".to_string(),
            reason: "validation manifest reported PASS WITH WARNINGS".to_string(),
        });
    }

    if !matches!(
        scope_manifest.status,
        ScopeCheckStatus::Pass | ScopeCheckStatus::PassWithWarnings
    ) {
        if scope_manifest.status == ScopeCheckStatus::ContractFail {
            return write_contract_fail_manifest(
                &repo_root,
                &manifest_path,
                build_contract_fail_manifest(
                    AdmissionManifestBase {
                        admission_run_id,
                        contract_path: contract_path_string,
                        contract_sha256,
                        repo_path: repo_path_string,
                        validation_manifest_path: validation_manifest_path_string,
                        scope_manifest_path: scope_manifest_path_string,
                        ledger_path: ledger_path_string,
                        started_unix_ms,
                    },
                    Some("scope_manifest_contract_fail".to_string()),
                ),
            );
        }
        violations.push(AdmissionViolation {
            kind: "scope_status_fail".to_string(),
            reason: format!("scope status {}", scope_manifest.status),
        });
    } else if scope_manifest.status == ScopeCheckStatus::PassWithWarnings {
        warnings.push(AdmissionWarning {
            kind: "scope_status_pass_with_warnings".to_string(),
            reason: "scope manifest reported PASS WITH WARNINGS".to_string(),
        });
    }

    let required_failed_commands = validation_manifest
        .commands
        .iter()
        .filter(|command| {
            command.required && command.status == crate::evidence::CommandStatus::Fail
        })
        .count();
    if required_failed_commands > 0 {
        violations.push(AdmissionViolation {
            kind: "required_validation_command_failed".to_string(),
            reason: format!(
                "{} required validation command(s) failed",
                required_failed_commands
            ),
        });
    }

    let missing_command_result_artifacts = validation_manifest
        .commands
        .iter()
        .filter(|command| {
            !resolve_input_path(&repo_root, Path::new(&command.result_path)).is_file()
        })
        .count();
    if missing_command_result_artifacts > 0 {
        violations.push(AdmissionViolation {
            kind: "missing_command_result_artifact".to_string(),
            reason: format!(
                "{} command result artifact(s) missing",
                missing_command_result_artifacts
            ),
        });
    }

    if !scope_manifest.violations.is_empty() {
        violations.push(AdmissionViolation {
            kind: "scope_violations_present".to_string(),
            reason: format!(
                "{} scope violation(s) recorded",
                scope_manifest.violations.len()
            ),
        });
    }

    let ledger_path = resolve_input_path(&repo_root, &request.ledger_path);
    let ledger_exists = ledger_path.is_file();
    let mut ledger_verification_status = None;
    let mut ledger_verification_manifest_path = None;

    if contract.ledger_update_required {
        let ledger_outcome = ledger::run_ledger_verification(ledger::LedgerVerificationRequest {
            contract_path: request.contract_path.clone(),
            repo: request.repo.clone(),
            ledger_path: request.ledger_path.clone(),
        })
        .map_err(|error| AdmissionVerdictError::Ledger(error.to_string()))?;

        ledger_verification_status = Some(ledger_outcome.manifest.status.to_string());
        ledger_verification_manifest_path = Some(ledger_outcome.manifest_path.clone());

        match ledger_outcome.manifest.status {
            LedgerVerificationStatus::Pass => {}
            LedgerVerificationStatus::PassWithWarnings => {
                warnings.extend(ledger_outcome.manifest.warnings.into_iter().map(|warning| {
                    AdmissionWarning {
                        kind: format!("ledger_{}", warning.kind),
                        reason: warning.reason,
                    }
                }));
            }
            LedgerVerificationStatus::Fail => {
                violations.push(AdmissionViolation {
                    kind: "ledger_verification_failed".to_string(),
                    reason: ledger_outcome
                        .manifest
                        .reason
                        .clone()
                        .unwrap_or_else(|| "ledger verification failed".to_string()),
                });
            }
            LedgerVerificationStatus::ContractFail => {
                return write_contract_fail_manifest(
                    &repo_root,
                    &manifest_path,
                    build_contract_fail_manifest(
                        AdmissionManifestBase {
                            admission_run_id,
                            contract_path: contract_path_string,
                            contract_sha256,
                            repo_path: repo_path_string,
                            validation_manifest_path: validation_manifest_path_string,
                            scope_manifest_path: scope_manifest_path_string,
                            ledger_path: ledger_path_string,
                            started_unix_ms,
                        },
                        Some(
                            ledger_outcome
                                .manifest
                                .reason
                                .clone()
                                .unwrap_or_else(|| "ledger_verification_contract_fail".to_string()),
                        ),
                    ),
                );
            }
        }
    }

    let evidence = AdmissionEvidenceSummary {
        validation_status: validation_manifest.status.to_string(),
        scope_status: scope_manifest.status.to_string(),
        validation_github_ci_used_as_evidence: validation_manifest.github_ci_used_as_evidence,
        scope_github_ci_used_as_evidence: scope_manifest.github_ci_used_as_evidence,
        scope_violations_count: scope_manifest.violations.len(),
        validation_commands_count: validation_manifest.commands.len(),
        required_validation_failures_count: required_failed_commands,
        missing_command_result_artifacts_count: missing_command_result_artifacts,
        ledger_verification_status,
        ledger_verification_manifest_path,
        ledger_exists,
        ledger_update_required: contract.ledger_update_required,
        contract_sha256_matches_validation: validation_contract_sha_matches,
        contract_sha256_matches_scope: scope_contract_sha_matches,
    };

    let status = if !violations.is_empty() {
        AdmissionStatus::Fail
    } else if !warnings.is_empty() {
        AdmissionStatus::PassWithWarnings
    } else {
        AdmissionStatus::Pass
    };

    let reason = if violations.is_empty() {
        None
    } else {
        Some(
            violations
                .first()
                .map(|violation| violation.kind.clone())
                .unwrap_or_else(|| "admission_violation".to_string()),
        )
    };

    let manifest = AdmissionVerdictManifest {
        schema_version: 1,
        admission_run_id,
        contract_path: contract_path_string,
        contract_sha256,
        repo_path: repo_path_string,
        validation_manifest_path: validation_manifest_path_string,
        scope_manifest_path: scope_manifest_path_string,
        ledger_path: ledger_path_string,
        status,
        started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        evidence,
        violations,
        warnings,
        decision_rule: "validation PASS + scope PASS + no hard violations".to_string(),
        reason,
    };

    write_manifest(&manifest_path, &manifest)?;
    Ok(outcome_from_manifest(&repo_root, manifest_path, manifest))
}

fn write_contract_fail_manifest(
    repo_root: &Path,
    manifest_path: &Path,
    manifest: AdmissionVerdictManifest,
) -> Result<AdmissionVerdictOutcome, AdmissionVerdictError> {
    write_manifest(manifest_path, &manifest)?;
    Ok(outcome_from_manifest(
        repo_root,
        manifest_path.to_path_buf(),
        manifest,
    ))
}

fn load_validation_manifest(path: &Path) -> Result<ValidationRunManifest, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("validation_manifest_read_failed: {}", error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("validation_manifest_parse_failed: {}", error))
}

fn load_scope_manifest(path: &Path) -> Result<ScopeCheckManifest, String> {
    let bytes = fs::read(path).map_err(|error| format!("scope_manifest_read_failed: {}", error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("scope_manifest_parse_failed: {}", error))
}

fn build_contract_fail_manifest(
    base: AdmissionManifestBase,
    reason: Option<String>,
) -> AdmissionVerdictManifest {
    AdmissionVerdictManifest {
        schema_version: 1,
        admission_run_id: base.admission_run_id,
        contract_path: base.contract_path,
        contract_sha256: base.contract_sha256,
        repo_path: base.repo_path,
        validation_manifest_path: base.validation_manifest_path,
        scope_manifest_path: base.scope_manifest_path,
        ledger_path: base.ledger_path,
        status: AdmissionStatus::ContractFail,
        started_unix_ms: base.started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        evidence: AdmissionEvidenceSummary {
            validation_status: "CONTRACT_FAIL".to_string(),
            scope_status: "CONTRACT_FAIL".to_string(),
            validation_github_ci_used_as_evidence: false,
            scope_github_ci_used_as_evidence: false,
            scope_violations_count: 0,
            validation_commands_count: 0,
            required_validation_failures_count: 0,
            missing_command_result_artifacts_count: 0,
            ledger_verification_status: None,
            ledger_verification_manifest_path: None,
            ledger_exists: false,
            ledger_update_required: false,
            contract_sha256_matches_validation: false,
            contract_sha256_matches_scope: false,
        },
        violations: vec![],
        warnings: vec![],
        decision_rule: "validation PASS + scope PASS + no hard violations".to_string(),
        reason,
    }
}

fn outcome_from_manifest(
    repo_root: &Path,
    manifest_path: PathBuf,
    manifest: AdmissionVerdictManifest,
) -> AdmissionVerdictOutcome {
    AdmissionVerdictOutcome {
        manifest,
        manifest_path: repo_relative_string(repo_root, &manifest_path),
    }
}

fn write_manifest(
    path: &Path,
    manifest: &AdmissionVerdictManifest,
) -> Result<(), AdmissionVerdictError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn generate_admission_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("admission-{}-{}", now, std::process::id())
}

fn resolve_input_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
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

fn normalize_path_string(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(path)
    }
}

impl fmt::Display for AdmissionVerdictManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::{ScopeCheckManifest, ScopeCheckStatus, ScopeCheckSummary, ScopeLimitStatus};
    use crate::validation_runner::{
        ValidationCommandCaptureEntry, ValidationRunManifest, ValidationRunStatus,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_admission_repo_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn contract_path(repo: &Path) -> PathBuf {
        repo.join("contract.json")
    }

    fn write_contract(repo: &Path, contract_json: &str) -> PathBuf {
        let path = contract_path(repo);
        fs::write(&path, contract_json).unwrap();
        path
    }

    fn sha256_hex_local(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex_encode(hasher.finalize())
    }

    fn contract_json(task_type: &str, ledger_update_required: bool) -> String {
        format!(
            r#"{{
  "project": "CCL",
  "workstream": "Admission",
  "task_type": "{}",
  "objective": "Admission verdict from evidence",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "docs/**", "examples/**", "ledger/**", "README.md", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["cargo fmt", "cargo test", "local admission guard"],
  "validation": {{
    "commands": [
      {{
        "id": "cargo-fmt",
        "program": "cargo",
        "args": ["fmt", "--check"],
        "required": true,
        "wall_timeout_seconds": 300
      }},
      {{
        "id": "cargo-test",
        "program": "cargo",
        "args": ["test"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "scope_limits": {{
    "max_changed_files": 25,
    "max_diff_lines": 1500
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": {},
  "verdicts": {{}}
}}"#,
            task_type, ledger_update_required
        )
    }

    fn contract_sha(contract: &Path) -> String {
        sha256_hex_local(&fs::read(contract).unwrap())
    }

    fn ensure_manifest_dirs(repo: &Path, run_id: &str) {
        fs::create_dir_all(repo.join(".ccl").join("runs").join(run_id).join("commands")).unwrap();
    }

    fn write_validation_manifest(
        repo: &Path,
        run_id: &str,
        contract: &Path,
        status: ValidationRunStatus,
        github_ci: bool,
        required: bool,
    ) -> PathBuf {
        ensure_manifest_dirs(repo, run_id);
        let result_path = repo
            .join(".ccl")
            .join("runs")
            .join(run_id)
            .join("commands")
            .join("001-cargo-test")
            .join("result.json");
        fs::create_dir_all(result_path.parent().unwrap()).unwrap();
        fs::write(&result_path, "{}").unwrap();
        let command_status = if required && matches!(status, ValidationRunStatus::Fail) {
            crate::evidence::CommandStatus::Fail
        } else {
            crate::evidence::CommandStatus::Pass
        };
        let manifest = ValidationRunManifest {
            schema_version: 1,
            validation_run_id: run_id.to_string(),
            contract_path: contract.to_string_lossy().into_owned(),
            contract_sha256: contract_sha(contract),
            repo_path: repo.to_string_lossy().into_owned(),
            status: status.clone(),
            started_unix_ms: 1,
            finished_unix_ms: 2,
            commands: vec![ValidationCommandCaptureEntry {
                id: "cargo-test".to_string(),
                required,
                status: command_status,
                capture_run_id: "capture-1".to_string(),
                result_path: repo
                    .join(".ccl")
                    .join("runs")
                    .join(run_id)
                    .join("commands")
                    .join("001-cargo-test")
                    .join("result.json")
                    .to_string_lossy()
                    .into_owned(),
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                exit_code: Some(if matches!(status, ValidationRunStatus::Fail) {
                    1
                } else {
                    0
                }),
                timed_out: false,
                output_limit_exceeded: false,
                failure_class: None,
            }],
            github_ci_used_as_evidence: github_ci,
            reason: None,
        };
        let manifest_path = repo
            .join(".ccl")
            .join("runs")
            .join(run_id)
            .join("validation-run-manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        manifest_path
    }

    fn write_scope_manifest(
        repo: &Path,
        run_id: &str,
        contract: &Path,
        status: ScopeCheckStatus,
        github_ci: bool,
        violations: Vec<AdmissionViolation>,
    ) -> PathBuf {
        ensure_manifest_dirs(repo, run_id);
        let manifest = ScopeCheckManifest {
            schema_version: 1,
            scope_run_id: run_id.to_string(),
            contract_path: contract.to_string_lossy().into_owned(),
            contract_sha256: contract_sha(contract),
            repo_path: repo.to_string_lossy().into_owned(),
            base_ref: "HEAD".to_string(),
            status,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            github_ci_used_as_evidence: github_ci,
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
            violations: violations
                .into_iter()
                .map(|violation| crate::scope::ScopeViolation {
                    kind: violation.kind,
                    path: String::new(),
                    reason: violation.reason,
                })
                .collect(),
            warnings: vec![],
            reason: None,
        };
        let manifest_path = repo
            .join(".ccl")
            .join("runs")
            .join(run_id)
            .join("scope-check-manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        manifest_path
    }

    fn write_ledger(repo: &Path) -> PathBuf {
        let ledger = repo.join("ledger");
        fs::create_dir_all(&ledger).unwrap();
        let ledger_path = ledger.join("project-ledger.md");
        fs::write(
            &ledger_path,
            r#"# CCL Project Ledger

## 2026-06-21 — Admission Verdict From Evidence Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: Admission
- Task type: guard_gate
- Branch: feat/admission-verdict-from-evidence-seed
- PR: #9
- Base main HEAD: 924a789e091c74beae4575c6346a8926cf0bc1e3

### Objective

- Objective: Compute an admission verdict from existing validation and scope evidence.

### Validation

- GitHub CI used as evidence: NO

### Next Gate

- recommended next gate: Gate Orchestration Seed
- reason: admission verdicts are now derived mechanically from evidence, so the next layer is a single orchestrator over the existing deterministic steps

### Admission Proof

- contract path: examples/ccl-admission-task-contract.json
- command: cargo run -p ccl-cli -- admission verdict --contract examples/ccl-admission-task-contract.json --repo . --validation-manifest .ccl/runs/validation-1782044715817-22052/validation-run-manifest.json --scope-manifest .ccl/runs/scope-1782044716205-33848/scope-check-manifest.json
- status: PASS
- admission verdict path: .ccl/runs/admission-1782044772197-20276/admission-verdict.json
- ledger verification manifest path: .ccl/runs/ledger-1782044661021-1332/ledger-verification-manifest.json

### Boundary Conclusion

- admission verdict command added: YES
- validation manifest consumed: YES
- scope manifest consumed: YES
- admission verdict invoked: YES
- GitHub CI used as evidence: NO

### Next Gate

- recommended next gate: External Review Intake / Threat Model Notes Seed
- reason: ledger semantics are now verified deterministically, so the next risk reduction step is review intake and threat modeling
"#,
        )
        .unwrap();
        ledger_path
    }

    fn admission_request(
        repo: &Path,
        contract: &Path,
        validation_manifest: &Path,
        scope_manifest: &Path,
        ledger_path: &Path,
    ) -> AdmissionVerdictRequest {
        AdmissionVerdictRequest {
            contract_path: contract.to_path_buf(),
            repo: repo.to_path_buf(),
            validation_manifest_path: validation_manifest.to_path_buf(),
            scope_manifest_path: scope_manifest.to_path_buf(),
            ledger_path: ledger_path.to_path_buf(),
        }
    }

    fn valid_result(
        repo: &Path,
        ledger_required: bool,
        validation_status: ValidationRunStatus,
        scope_status: ScopeCheckStatus,
        validation_ci: bool,
        scope_ci: bool,
        ledger_present: bool,
    ) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let contract = write_contract(repo, &contract_json("guard_gate", ledger_required));
        let validation_manifest = write_validation_manifest(
            repo,
            "validation-1",
            &contract,
            validation_status,
            validation_ci,
            true,
        );
        let scope_manifest =
            write_scope_manifest(repo, "scope-1", &contract, scope_status, scope_ci, vec![]);
        let ledger_path = if ledger_present {
            write_ledger(repo)
        } else {
            repo.join("ledger").join("project-ledger.md")
        };
        (contract, validation_manifest, scope_manifest, ledger_path)
    }

    #[test]
    fn validation_pass_and_scope_pass_yield_pass_status_when_ledger_exists() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Pass,
            ScopeCheckStatus::Pass,
            false,
            false,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Pass);
        assert!(!outcome
            .manifest
            .warnings
            .iter()
            .any(|warning| warning.kind == "ledger_semantic_verification_not_implemented"));
        assert!(Path::new(&repo.join(outcome.manifest_path)).exists());
    }

    #[test]
    fn validation_fail_yields_fail() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Fail,
            ScopeCheckStatus::Pass,
            false,
            false,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "validation_status_fail"));
    }

    #[test]
    fn scope_fail_yields_fail() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Pass,
            ScopeCheckStatus::Fail,
            false,
            false,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "scope_status_fail"));
    }

    #[test]
    fn scope_violations_yield_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let validation_manifest = write_validation_manifest(
            &repo,
            "validation-1",
            &contract,
            ValidationRunStatus::Pass,
            false,
            true,
        );
        let scope_manifest = write_scope_manifest(
            &repo,
            "scope-1",
            &contract,
            ScopeCheckStatus::Pass,
            false,
            vec![AdmissionViolation {
                kind: "forbidden_path_changed".to_string(),
                reason: "matched forbidden path .github/**".to_string(),
            }],
        );
        let ledger_path = write_ledger(&repo);

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "scope_violations_present"));
    }

    #[test]
    fn github_ci_used_as_evidence_in_validation_fails() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Pass,
            ScopeCheckStatus::Pass,
            true,
            false,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "validation_github_ci_used_as_evidence"));
    }

    #[test]
    fn github_ci_used_as_evidence_in_scope_fails() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Pass,
            ScopeCheckStatus::Pass,
            false,
            true,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "scope_github_ci_used_as_evidence"));
    }

    #[test]
    fn missing_validation_manifest_yields_contract_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let scope_manifest = write_scope_manifest(
            &repo,
            "scope-1",
            &contract,
            ScopeCheckStatus::Pass,
            false,
            vec![],
        );
        let ledger_path = write_ledger(&repo);
        let validation_manifest = repo.join(".ccl/runs/missing/validation-run-manifest.json");

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("validation_manifest_read_failed"));
    }

    #[test]
    fn invalid_validation_manifest_yields_contract_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let scope_manifest = write_scope_manifest(
            &repo,
            "scope-1",
            &contract,
            ScopeCheckStatus::Pass,
            false,
            vec![],
        );
        let ledger_path = write_ledger(&repo);
        let validation_manifest = repo
            .join(".ccl")
            .join("runs")
            .join("bad")
            .join("validation-run-manifest.json");
        fs::create_dir_all(validation_manifest.parent().unwrap()).unwrap();
        fs::write(&validation_manifest, "{ not json").unwrap();

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("validation_manifest_parse_failed"));
    }

    #[test]
    fn missing_scope_manifest_yields_contract_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let validation_manifest = write_validation_manifest(
            &repo,
            "validation-1",
            &contract,
            ValidationRunStatus::Pass,
            false,
            true,
        );
        let ledger_path = write_ledger(&repo);
        let scope_manifest = repo.join(".ccl/runs/missing/scope-check-manifest.json");

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("scope_manifest_read_failed"));
    }

    #[test]
    fn invalid_scope_manifest_yields_contract_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let validation_manifest = write_validation_manifest(
            &repo,
            "validation-1",
            &contract,
            ValidationRunStatus::Pass,
            false,
            true,
        );
        let ledger_path = write_ledger(&repo);
        let scope_manifest = repo
            .join(".ccl")
            .join("runs")
            .join("bad")
            .join("scope-check-manifest.json");
        fs::create_dir_all(scope_manifest.parent().unwrap()).unwrap();
        fs::write(&scope_manifest, "{ not json").unwrap();

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("scope_manifest_parse_failed"));
    }

    #[test]
    fn contract_hash_mismatch_yields_contract_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let different_contract = repo.join("other-contract.json");
        fs::write(&different_contract, contract_json("audit_only", true)).unwrap();
        let validation_manifest = write_validation_manifest(
            &repo,
            "validation-1",
            &contract,
            ValidationRunStatus::Pass,
            false,
            true,
        );
        let scope_manifest = write_scope_manifest(
            &repo,
            "scope-1",
            &contract,
            ScopeCheckStatus::Pass,
            false,
            vec![],
        );
        let ledger_path = write_ledger(&repo);

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &different_contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("validation_manifest_contract_sha256_mismatch"));
    }

    #[test]
    fn missing_ledger_when_required_yields_fail() {
        let repo = repo_dir();
        let contract = write_contract(&repo, &contract_json("guard_gate", true));
        let validation_manifest = write_validation_manifest(
            &repo,
            "validation-1",
            &contract,
            ValidationRunStatus::Pass,
            false,
            true,
        );
        let scope_manifest = write_scope_manifest(
            &repo,
            "scope-1",
            &contract,
            ScopeCheckStatus::Pass,
            false,
            vec![],
        );
        let ledger_path = repo.join("ledger").join("project-ledger.md");

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::ContractFail);
        assert!(outcome
            .manifest
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("ledger_missing"));
    }

    #[test]
    fn admission_verdict_manifest_is_written_and_ci_flag_false() {
        let repo = repo_dir();
        let (contract, validation_manifest, scope_manifest, ledger_path) = valid_result(
            &repo,
            true,
            ValidationRunStatus::Pass,
            ScopeCheckStatus::Pass,
            false,
            false,
            true,
        );

        let outcome = run_admission_verdict(admission_request(
            &repo,
            &contract,
            &validation_manifest,
            &scope_manifest,
            &ledger_path,
        ))
        .unwrap();

        assert!(Path::new(&repo.join(outcome.manifest_path)).exists());
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }
}
