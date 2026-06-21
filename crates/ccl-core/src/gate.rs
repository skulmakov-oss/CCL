use crate::admission::{self, AdmissionVerdictOutcome, AdmissionVerdictRequest};
use crate::scope::{self, ScopeCheckOutcome, ScopeCheckStatus};
use crate::validation_runner::{self, ValidationRunOutcome, ValidationRunStatus};
use crate::verdict::AdmissionStatus;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type GateRunStatus = AdmissionStatus;

#[derive(Debug, Clone)]
pub struct GateRunRequest {
    pub contract_path: PathBuf,
    pub repo: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateStepName {
    Validation,
    Scope,
    Admission,
}

impl fmt::Display for GateStepName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateStepName::Validation => write!(f, "validation"),
            GateStepName::Scope => write!(f, "scope"),
            GateStepName::Admission => write!(f, "admission"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStepManifest {
    pub name: GateStepName,
    pub status: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateViolation {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateWarning {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRunManifest {
    pub schema_version: u32,
    pub gate_run_id: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub repo_path: String,
    pub status: GateRunStatus,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
    pub github_ci_used_as_evidence: bool,
    #[serde(default)]
    pub steps: Vec<GateStepManifest>,
    #[serde(default)]
    pub warnings: Vec<GateWarning>,
    #[serde(default)]
    pub violations: Vec<GateViolation>,
}

#[derive(Debug, Clone)]
pub struct GateRunOutcome {
    pub manifest: GateRunManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GateRunError {
    #[error("validation runner error: {0}")]
    Validation(String),
    #[error("scope checker error: {0}")]
    Scope(String),
    #[error("admission verdict error: {0}")]
    Admission(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn run_gate(request: GateRunRequest) -> Result<GateRunOutcome, GateRunError> {
    run_gate_with_steps(
        request,
        |contract_path, repo| {
            validation_runner::run_validation(contract_path, repo)
                .map_err(|error| GateRunError::Validation(error.to_string()))
        },
        |contract_path, repo| {
            scope::run_scope_check(contract_path, repo)
                .map_err(|error| GateRunError::Scope(error.to_string()))
        },
        |admission_request| {
            admission::run_admission_verdict(admission_request)
                .map_err(|error| GateRunError::Admission(error.to_string()))
        },
    )
}

fn run_gate_with_steps<V, S, A>(
    request: GateRunRequest,
    mut run_validation: V,
    mut run_scope: S,
    mut run_admission: A,
) -> Result<GateRunOutcome, GateRunError>
where
    V: FnMut(&Path, &Path) -> Result<ValidationRunOutcome, GateRunError>,
    S: FnMut(&Path, &Path) -> Result<ScopeCheckOutcome, GateRunError>,
    A: FnMut(AdmissionVerdictRequest) -> Result<AdmissionVerdictOutcome, GateRunError>,
{
    let gate_run_id = generate_gate_run_id();
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_dir = repo_root.join(".ccl").join("runs").join(&gate_run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("gate-run-manifest.json");
    let started_unix_ms = system_time_ms(SystemTime::now());

    let repo_path_string = request.repo.to_string_lossy().into_owned();
    let contract_path_string = request.contract_path.to_string_lossy().into_owned();

    let validation_outcome = run_validation(&request.contract_path, &request.repo)?;
    let scope_outcome = run_scope(&request.contract_path, &request.repo)?;
    let admission_outcome = run_admission(AdmissionVerdictRequest {
        contract_path: request.contract_path.clone(),
        repo: request.repo.clone(),
        validation_manifest_path: PathBuf::from(&validation_outcome.manifest_path),
        scope_manifest_path: PathBuf::from(&scope_outcome.manifest_path),
        ledger_path: request.repo.join("ledger/project-ledger.md"),
    })?;

    let mut warnings = Vec::new();
    if validation_outcome.manifest.status == ValidationRunStatus::PassWithWarnings {
        warnings.push(GateWarning {
            kind: "validation_status_pass_with_warnings".to_string(),
            reason: "validation manifest reported PASS WITH WARNINGS".to_string(),
        });
    }
    if scope_outcome.manifest.status == ScopeCheckStatus::PassWithWarnings {
        warnings.push(GateWarning {
            kind: "scope_status_pass_with_warnings".to_string(),
            reason: "scope manifest reported PASS WITH WARNINGS".to_string(),
        });
    }
    warnings.extend(
        admission_outcome
            .manifest
            .warnings
            .iter()
            .map(|warning| GateWarning {
                kind: warning.kind.clone(),
                reason: warning.reason.clone(),
            }),
    );

    let mut violations = admission_outcome
        .manifest
        .violations
        .iter()
        .map(|violation| GateViolation {
            kind: violation.kind.clone(),
            reason: violation.reason.clone(),
        })
        .collect::<Vec<_>>();

    if violations.is_empty()
        && matches!(
            admission_outcome.manifest.status,
            AdmissionStatus::Fail | AdmissionStatus::ContractFail
        )
    {
        violations.push(GateViolation {
            kind: match admission_outcome.manifest.status {
                AdmissionStatus::Fail => "admission_status_fail".to_string(),
                AdmissionStatus::ContractFail => "admission_status_contract_fail".to_string(),
                _ => "admission_status_fail".to_string(),
            },
            reason: admission_outcome
                .manifest
                .reason
                .clone()
                .unwrap_or_else(|| admission_outcome.manifest.status.to_string()),
        });
    }

    let manifest = GateRunManifest {
        schema_version: 1,
        gate_run_id,
        contract_path: contract_path_string,
        contract_sha256: validation_outcome.manifest.contract_sha256.clone(),
        repo_path: repo_path_string,
        status: admission_outcome.manifest.status.clone(),
        started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: validation_outcome.manifest.github_ci_used_as_evidence
            || scope_outcome.manifest.github_ci_used_as_evidence
            || admission_outcome.manifest.github_ci_used_as_evidence,
        steps: vec![
            GateStepManifest {
                name: GateStepName::Validation,
                status: validation_outcome.manifest.status.to_string(),
                manifest_path: validation_outcome.manifest_path.clone(),
            },
            GateStepManifest {
                name: GateStepName::Scope,
                status: scope_outcome.manifest.status.to_string(),
                manifest_path: scope_outcome.manifest_path.clone(),
            },
            GateStepManifest {
                name: GateStepName::Admission,
                status: admission_outcome.manifest.status.to_string(),
                manifest_path: admission_outcome.manifest_path.clone(),
            },
        ],
        warnings,
        violations,
    };

    write_manifest(&manifest_path, &manifest)?;
    Ok(GateRunOutcome {
        manifest,
        manifest_path: repo_relative_string(&repo_root, &manifest_path),
    })
}

fn write_manifest(path: &Path, manifest: &GateRunManifest) -> Result<(), GateRunError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn generate_gate_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("gate-{}-{}", now, std::process::id())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::{
        AdmissionEvidenceSummary, AdmissionVerdictManifest, AdmissionViolation, AdmissionWarning,
    };
    use crate::evidence::CommandStatus;
    use crate::scope::{ScopeCheckManifest, ScopeCheckSummary, ScopeLimitStatus, ScopeViolation};
    use crate::validation_runner::{ValidationCommandCaptureEntry, ValidationRunManifest};
    use hex::encode as hex_encode;
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_gate_repo_{}_{}_{}",
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

    fn helper_exe() -> PathBuf {
        static HELPER: OnceLock<PathBuf> = OnceLock::new();
        HELPER
            .get_or_init(|| {
                let root = std::env::temp_dir().join(format!(
                    "ccl_gate_helper_{}_{}",
                    std::process::id(),
                    system_time_ms(SystemTime::now())
                ));
                fs::create_dir_all(&root).unwrap();
                let src = root.join("helper.rs");
                let exe = root.join(format!("helper{}", if cfg!(windows) { ".exe" } else { "" }));
                fs::write(
                    &src,
                    r#"
use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "pass" => println!("pass"),
        "fail" => {
            eprintln!("fail");
            std::process::exit(7);
        }
        "sleep" => thread::sleep(Duration::from_secs(5)),
        _ => println!("unknown"),
    }
}
"#,
                )
                .unwrap();
                let status = Command::new("rustc")
                    .arg(&src)
                    .arg("-O")
                    .arg("-o")
                    .arg(&exe)
                    .status()
                    .unwrap();
                assert!(status.success());
                exe
            })
            .clone()
    }

    fn json_escape_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }

    fn sha256_hex_local(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex_encode(hasher.finalize())
    }

    fn contract_sha(contract: &Path) -> String {
        sha256_hex_local(&fs::read(contract).unwrap())
    }

    fn helper_contract(helper: &Path, command_mode: &str) -> PathBuf {
        let helper = json_escape_path(helper);
        let contract = std::env::temp_dir().join(format!(
            "ccl_gate_contract_{}_{}_{}.json",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::write(
            &contract,
            format!(
                r#"{{
  "project": "CCL",
  "workstream": "Gate",
  "task_type": "guard_gate",
  "objective": "Gate orchestration",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["src/**", "ledger/**", ".gitignore", "README.md", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["helper validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-{}",
        "program": "{}",
        "args": ["{}"],
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
  "ledger_update_required": true,
  "verdicts": {{}}
}}"#,
                command_mode, helper, command_mode
            ),
        )
        .unwrap();
        contract
    }

    fn init_clean_repo_with_ledger_and_gitignore() -> PathBuf {
        let repo = repo_dir();
        init_git_repo(&repo);
        fs::write(repo.join(".gitignore"), ".ccl/\n").unwrap();
        write_commit_file(&repo, ".gitignore", ".ccl/\n");
        write_commit_file(&repo, "src/app.rs", "fn main() {}\n");
        write_commit_file(&repo, "ledger/project-ledger.md", "# Ledger\n");
        repo
    }

    fn stub_validation_outcome(
        repo: &Path,
        contract: &Path,
        status: ValidationRunStatus,
    ) -> ValidationRunOutcome {
        ValidationRunOutcome {
            manifest: ValidationRunManifest {
                schema_version: 1,
                validation_run_id: "validation-1".to_string(),
                contract_path: contract.to_string_lossy().into_owned(),
                contract_sha256: contract_sha(contract),
                repo_path: repo.to_string_lossy().into_owned(),
                status,
                started_unix_ms: 1,
                finished_unix_ms: 2,
                commands: vec![ValidationCommandCaptureEntry {
                    id: "helper".to_string(),
                    required: true,
                    status: CommandStatus::Pass,
                    capture_run_id: "capture-1".to_string(),
                    result_path: ".ccl/runs/validation-1/commands/001-helper/result.json"
                        .to_string(),
                    stdout_sha256: "stdout".to_string(),
                    stderr_sha256: "stderr".to_string(),
                    exit_code: Some(0),
                    timed_out: false,
                    output_limit_exceeded: false,
                    failure_class: None,
                }],
                github_ci_used_as_evidence: false,
                reason: None,
            },
            manifest_path: ".ccl/runs/validation-1/validation-run-manifest.json".to_string(),
        }
    }

    fn stub_scope_outcome(
        repo: &Path,
        contract: &Path,
        status: ScopeCheckStatus,
    ) -> ScopeCheckOutcome {
        ScopeCheckOutcome {
            manifest: ScopeCheckManifest {
                schema_version: 1,
                scope_run_id: "scope-1".to_string(),
                contract_path: contract.to_string_lossy().into_owned(),
                contract_sha256: contract_sha(contract),
                repo_path: repo.to_string_lossy().into_owned(),
                base_ref: "HEAD".to_string(),
                status,
                started_unix_ms: 1,
                finished_unix_ms: 2,
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
                violations: vec![ScopeViolation {
                    kind: "scope_violation".to_string(),
                    path: String::new(),
                    reason: "scope".to_string(),
                }],
                warnings: vec![],
                reason: None,
            },
            manifest_path: ".ccl/runs/scope-1/scope-check-manifest.json".to_string(),
        }
    }

    fn stub_admission_outcome(
        repo: &Path,
        contract: &Path,
        validation_manifest_path: &str,
        scope_manifest_path: &str,
        status: AdmissionStatus,
        warnings: Vec<AdmissionWarning>,
        violations: Vec<AdmissionViolation>,
    ) -> AdmissionVerdictOutcome {
        AdmissionVerdictOutcome {
            manifest: AdmissionVerdictManifest {
                schema_version: 1,
                admission_run_id: "admission-1".to_string(),
                contract_path: contract.to_string_lossy().into_owned(),
                contract_sha256: contract_sha(contract),
                repo_path: repo.to_string_lossy().into_owned(),
                validation_manifest_path: validation_manifest_path.to_string(),
                scope_manifest_path: scope_manifest_path.to_string(),
                ledger_path: repo
                    .join("ledger/project-ledger.md")
                    .to_string_lossy()
                    .into_owned(),
                status,
                started_unix_ms: 1,
                finished_unix_ms: 2,
                github_ci_used_as_evidence: false,
                evidence: AdmissionEvidenceSummary {
                    validation_status: "PASS".to_string(),
                    scope_status: "PASS".to_string(),
                    validation_github_ci_used_as_evidence: false,
                    scope_github_ci_used_as_evidence: false,
                    scope_violations_count: 0,
                    validation_commands_count: 1,
                    required_validation_failures_count: 0,
                    missing_command_result_artifacts_count: 0,
                    ledger_exists: true,
                    ledger_update_required: true,
                    contract_sha256_matches_validation: true,
                    contract_sha256_matches_scope: true,
                },
                violations,
                warnings,
                decision_rule: "validation PASS + scope PASS + no hard violations".to_string(),
                reason: None,
            },
            manifest_path: ".ccl/runs/admission-1/admission-verdict.json".to_string(),
        }
    }

    #[test]
    fn gate_calls_validation_scope_and_admission_in_sequence() {
        let repo = repo_dir();
        let contract = helper_contract(&helper_exe(), "pass");
        let repo_for_validation = repo.clone();
        let repo_for_scope = repo.clone();
        let repo_for_admission = repo.clone();
        let contract_for_validation = contract.clone();
        let contract_for_scope = contract.clone();
        let contract_for_admission = contract.clone();
        let order = Rc::new(RefCell::new(Vec::new()));
        let validation_order = Rc::clone(&order);
        let scope_order = Rc::clone(&order);
        let admission_order = Rc::clone(&order);

        let outcome = run_gate_with_steps(
            GateRunRequest {
                contract_path: contract.clone(),
                repo: repo.clone(),
            },
            move |contract_path, repo_path| {
                validation_order.borrow_mut().push("validation");
                assert_eq!(contract_path, contract_for_validation.as_path());
                assert_eq!(repo_path, repo_for_validation.as_path());
                Ok(stub_validation_outcome(
                    repo_path,
                    contract_path,
                    ValidationRunStatus::Pass,
                ))
            },
            move |contract_path, repo_path| {
                scope_order.borrow_mut().push("scope");
                assert_eq!(contract_path, contract_for_scope.as_path());
                assert_eq!(repo_path, repo_for_scope.as_path());
                Ok(stub_scope_outcome(
                    repo_path,
                    contract_path,
                    ScopeCheckStatus::Pass,
                ))
            },
            move |admission_request| {
                admission_order.borrow_mut().push("admission");
                assert_eq!(admission_request.contract_path, contract_for_admission);
                assert_eq!(admission_request.repo, repo_for_admission);
                assert_eq!(
                    admission_request.validation_manifest_path,
                    PathBuf::from(".ccl/runs/validation-1/validation-run-manifest.json")
                );
                assert_eq!(
                    admission_request.scope_manifest_path,
                    PathBuf::from(".ccl/runs/scope-1/scope-check-manifest.json")
                );
                Ok(stub_admission_outcome(
                    &repo_for_admission,
                    &admission_request.contract_path,
                    ".ccl/runs/validation-1/validation-run-manifest.json",
                    ".ccl/runs/scope-1/scope-check-manifest.json",
                    AdmissionStatus::PassWithWarnings,
                    vec![AdmissionWarning {
                        kind: "ledger_semantic_verification_not_implemented".to_string(),
                        reason: "ledger semantic verification not implemented yet".to_string(),
                    }],
                    vec![],
                ))
            },
        )
        .unwrap();

        assert_eq!(
            order.borrow().as_slice(),
            &["validation", "scope", "admission"]
        );
        assert_eq!(outcome.manifest.status, AdmissionStatus::PassWithWarnings);
        assert_eq!(outcome.manifest.steps.len(), 3);
        assert!(Path::new(&repo.join(&outcome.manifest_path)).exists());
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn gate_status_mirrors_admission_status() {
        let repo = init_clean_repo_with_ledger_and_gitignore();
        let helper = helper_exe();
        let contract = helper_contract(&helper, "pass");
        let outcome = run_gate(GateRunRequest {
            contract_path: contract,
            repo: repo.clone(),
        })
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::PassWithWarnings);
        assert_eq!(outcome.manifest.steps.len(), 3);
        assert_eq!(outcome.manifest.steps[0].name, GateStepName::Validation);
        assert_eq!(outcome.manifest.steps[1].name, GateStepName::Scope);
        assert_eq!(outcome.manifest.steps[2].name, GateStepName::Admission);
        assert!(Path::new(&repo.join(&outcome.manifest_path)).exists());
    }

    #[test]
    fn validation_failure_leads_to_gate_fail() {
        let repo = init_clean_repo_with_ledger_and_gitignore();
        let helper = helper_exe();
        let contract = helper_contract(&helper, "fail");
        let outcome = run_gate(GateRunRequest {
            contract_path: contract,
            repo: repo.clone(),
        })
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert_eq!(outcome.manifest.steps[0].status, "FAIL");
        assert_eq!(outcome.manifest.steps[2].status, "FAIL");
        assert!(Path::new(&repo.join(&outcome.manifest_path)).exists());
    }

    #[test]
    fn scope_failure_leads_to_gate_fail() {
        let repo = init_clean_repo_with_ledger_and_gitignore();
        let debug_path = repo.join("tmp/debug.log");
        if let Some(parent) = debug_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&debug_path, "debug\n").unwrap();
        let helper = helper_exe();
        let contract = helper_contract(&helper, "pass");
        let outcome = run_gate(GateRunRequest {
            contract_path: contract,
            repo: repo.clone(),
        })
        .unwrap();

        assert_eq!(outcome.manifest.status, AdmissionStatus::Fail);
        assert_eq!(outcome.manifest.steps[1].status, "FAIL");
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "outside_allowed_scope"
                || violation.kind == "scope_status_fail"
                || violation.kind == "scope_violations_present"));
    }

    #[test]
    fn gate_manifest_records_all_manifest_paths() {
        let repo = init_clean_repo_with_ledger_and_gitignore();
        let helper = helper_exe();
        let contract = helper_contract(&helper, "pass");
        let outcome = run_gate(GateRunRequest {
            contract_path: contract,
            repo: repo.clone(),
        })
        .unwrap();

        assert!(outcome
            .manifest
            .steps
            .iter()
            .any(|step| step.name == GateStepName::Validation
                && step.manifest_path.starts_with(".ccl/runs/validation-")));
        assert!(outcome
            .manifest
            .steps
            .iter()
            .any(|step| step.name == GateStepName::Scope
                && step.manifest_path.starts_with(".ccl/runs/scope-")));
        assert!(outcome
            .manifest
            .steps
            .iter()
            .any(|step| step.name == GateStepName::Admission
                && step.manifest_path.starts_with(".ccl/runs/admission-")));
        assert!(outcome
            .manifest
            .steps
            .iter()
            .all(|step| step.manifest_path.contains("/")));
    }

    #[test]
    fn gate_manifest_records_github_ci_false() {
        let repo = init_clean_repo_with_ledger_and_gitignore();
        let helper = helper_exe();
        let contract = helper_contract(&helper, "pass");
        let outcome = run_gate(GateRunRequest {
            contract_path: contract,
            repo: repo.clone(),
        })
        .unwrap();

        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn gate_internal_validation_error_bubbles_up() {
        let repo = repo_dir();
        let contract = repo.join("contract.json");
        fs::write(&contract, "{}").unwrap();
        let order = Rc::new(RefCell::new(Vec::new()));
        let validation_order = Rc::clone(&order);

        let result = run_gate_with_steps(
            GateRunRequest {
                contract_path: contract,
                repo,
            },
            move |_contract_path, _repo_path| {
                validation_order.borrow_mut().push("validation");
                Err(GateRunError::Validation("boom".to_string()))
            },
            |_contract_path, _repo_path| {
                unreachable!("scope should not run after validation error")
            },
            |_admission_request| unreachable!("admission should not run after validation error"),
        );

        assert!(result.is_err());
        assert_eq!(order.borrow().as_slice(), &["validation"]);
    }
}
