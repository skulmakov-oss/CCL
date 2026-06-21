use crate::capture::{capture_command, CaptureError};
use crate::environment::{EnvironmentPolicy, EnvironmentPolicyMode, EnvironmentPolicyStatus};
use crate::evidence::{
    CapturePolicy, CaptureRequest, CommandStatus, FailureClass, OutputLimitPolicy,
};
use crate::task_contract::{TaskContract, ValidationCommand};
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationRunStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for ValidationRunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationRunStatus::Pass => write!(f, "PASS"),
            ValidationRunStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            ValidationRunStatus::Fail => write!(f, "FAIL"),
            ValidationRunStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCommandCaptureEntry {
    pub id: String,
    pub required: bool,
    pub status: CommandStatus,
    pub capture_run_id: String,
    pub result_path: String,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_limit_exceeded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<FailureClass>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEnvironmentPolicyCommandResult {
    pub command_id: String,
    pub status: EnvironmentPolicyStatus,
    pub warnings_count: usize,
    pub violations_count: usize,
    pub redacted_variables_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEnvironmentPolicySummary {
    pub mode: EnvironmentPolicyMode,
    pub status: EnvironmentPolicyStatus,
    pub checked: bool,
    #[serde(default)]
    pub command_results: Vec<ValidationEnvironmentPolicyCommandResult>,
    pub warnings_count: usize,
    pub violations_count: usize,
    pub redacted_variables_count: usize,
}

impl Default for ValidationEnvironmentPolicySummary {
    fn default() -> Self {
        Self {
            mode: EnvironmentPolicyMode::RecordOnly,
            status: EnvironmentPolicyStatus::Pass,
            checked: false,
            command_results: vec![],
            warnings_count: 0,
            violations_count: 0,
            redacted_variables_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRunManifest {
    pub schema_version: u32,
    pub validation_run_id: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub repo_path: String,
    pub status: ValidationRunStatus,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
    #[serde(default)]
    pub commands: Vec<ValidationCommandCaptureEntry>,
    pub github_ci_used_as_evidence: bool,
    #[serde(default)]
    pub environment_policy: ValidationEnvironmentPolicySummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationRunOutcome {
    pub manifest: ValidationRunManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationRunnerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("capture error: {0}")]
    Capture(#[from] CaptureError),
}

pub fn run_validation(
    contract_path: &Path,
    repo: &Path,
) -> Result<ValidationRunOutcome, ValidationRunnerError> {
    let validation_run_id = generate_validation_run_id();
    let repo_root = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let run_dir = repo_root.join(".ccl").join("runs").join(&validation_run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("validation-run-manifest.json");
    let started_unix_ms = system_time_ms(SystemTime::now());

    let repo_path_string = repo.to_string_lossy().into_owned();
    let contract_path_string = contract_path.to_string_lossy().into_owned();

    let contract_bytes = match fs::read(contract_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let manifest = ValidationRunManifest {
                schema_version: 1,
                validation_run_id,
                contract_path: contract_path_string,
                contract_sha256: String::new(),
                repo_path: repo_path_string.clone(),
                status: ValidationRunStatus::ContractFail,
                started_unix_ms,
                finished_unix_ms: system_time_ms(SystemTime::now()),
                commands: vec![],
                github_ci_used_as_evidence: false,
                environment_policy: ValidationEnvironmentPolicySummary::default(),
                reason: Some(format!("contract_read_failed: {}", error)),
            };
            write_manifest(&manifest_path, &manifest)?;
            return Ok(ValidationRunOutcome {
                manifest,
                manifest_path: repo_relative_string(&repo_root, &manifest_path),
            });
        }
    };

    let contract_sha256 = sha256_hex(&contract_bytes);
    let contract = match serde_json::from_slice::<TaskContract>(&contract_bytes) {
        Ok(contract) => contract,
        Err(error) => {
            let manifest = ValidationRunManifest {
                schema_version: 1,
                validation_run_id,
                contract_path: contract_path_string,
                contract_sha256,
                repo_path: repo_path_string.clone(),
                status: ValidationRunStatus::ContractFail,
                started_unix_ms,
                finished_unix_ms: system_time_ms(SystemTime::now()),
                commands: vec![],
                github_ci_used_as_evidence: false,
                environment_policy: ValidationEnvironmentPolicySummary::default(),
                reason: Some(format!("contract_parse_failed: {}", error)),
            };
            write_manifest(&manifest_path, &manifest)?;
            return Ok(ValidationRunOutcome {
                manifest,
                manifest_path: repo_relative_string(&repo_root, &manifest_path),
            });
        }
    };

    let contract_report = contract.validate();
    if contract_report.status.is_failure() {
        let manifest = ValidationRunManifest {
            schema_version: 1,
            validation_run_id,
            contract_path: contract_path_string,
            contract_sha256,
            repo_path: repo_path_string.clone(),
            status: ValidationRunStatus::ContractFail,
            started_unix_ms,
            finished_unix_ms: system_time_ms(SystemTime::now()),
            commands: vec![],
            github_ci_used_as_evidence: false,
            environment_policy: ValidationEnvironmentPolicySummary::default(),
            reason: Some("contract_validation_failed".to_string()),
        };
        write_manifest(&manifest_path, &manifest)?;
        return Ok(ValidationRunOutcome {
            manifest,
            manifest_path: repo_relative_string(&repo_root, &manifest_path),
        });
    }

    let validation_commands = contract.validation.commands.clone();
    if validation_commands.is_empty() {
        let manifest = ValidationRunManifest {
            schema_version: 1,
            validation_run_id,
            contract_path: contract_path_string,
            contract_sha256,
            repo_path: repo_path_string.clone(),
            status: ValidationRunStatus::ContractFail,
            started_unix_ms,
            finished_unix_ms: system_time_ms(SystemTime::now()),
            commands: vec![],
            github_ci_used_as_evidence: false,
            environment_policy: ValidationEnvironmentPolicySummary::default(),
            reason: Some("no_validation_commands".to_string()),
        };
        write_manifest(&manifest_path, &manifest)?;
        return Ok(ValidationRunOutcome {
            manifest,
            manifest_path: repo_relative_string(&repo_root, &manifest_path),
        });
    }

    let mut command_entries = Vec::new();
    let mut has_optional_failure = false;
    let mut has_env_warning = false;
    let mut has_env_failure = false;
    let mut has_env_contract_fail = false;
    let mut final_status = ValidationRunStatus::Pass;
    let mut final_reason: Option<String> = None;
    let mut environment_policy_summary = ValidationEnvironmentPolicySummary::default();
    let effective_environment_policy = contract
        .environment_policy
        .clone()
        .unwrap_or_else(EnvironmentPolicy::default);
    environment_policy_summary.mode = effective_environment_policy.mode.clone();

    for validation_command in validation_commands {
        let capture_outcome = capture_command(build_capture_request(
            &repo_root,
            &validation_command,
            &effective_environment_policy,
        ))?;
        let capture_result = capture_outcome.command_result;
        let capture_run_id = capture_outcome.run.run_id.clone();

        let env_policy_result = capture_result.environment_policy.clone();
        environment_policy_summary.checked = true;
        environment_policy_summary.mode = env_policy_result.mode.clone();
        environment_policy_summary.warnings_count = environment_policy_summary
            .warnings_count
            .saturating_add(env_policy_result.warnings.len());
        environment_policy_summary.violations_count = environment_policy_summary
            .violations_count
            .saturating_add(env_policy_result.violations.len());
        environment_policy_summary.redacted_variables_count = environment_policy_summary
            .redacted_variables_count
            .saturating_add(env_policy_result.redacted_variables.len());
        environment_policy_summary.status = aggregate_environment_policy_status(
            &environment_policy_summary.status,
            &env_policy_result.status,
        );
        environment_policy_summary
            .command_results
            .push(ValidationEnvironmentPolicyCommandResult {
                command_id: validation_command.id.clone(),
                status: env_policy_result.status.clone(),
                warnings_count: env_policy_result.warnings.len(),
                violations_count: env_policy_result.violations.len(),
                redacted_variables_count: env_policy_result.redacted_variables.len(),
            });

        match env_policy_result.status {
            EnvironmentPolicyStatus::Pass => {}
            EnvironmentPolicyStatus::Warn => {
                has_env_warning = true;
            }
            EnvironmentPolicyStatus::Fail => {
                has_env_failure = true;
            }
            EnvironmentPolicyStatus::ContractFail => {
                has_env_contract_fail = true;
            }
        }

        let entry = ValidationCommandCaptureEntry {
            id: capture_result.id.clone(),
            required: validation_command.required,
            status: capture_result.status.clone(),
            capture_run_id,
            result_path: repo_relative_string(
                repo,
                normalize_path_string(&capture_result.result_path),
            ),
            stdout_sha256: capture_result.stdout.sha256.clone(),
            stderr_sha256: capture_result.stderr.sha256.clone(),
            exit_code: capture_result.exit_code,
            timed_out: capture_result.timed_out,
            output_limit_exceeded: capture_result.output_limit_exceeded,
            failure_class: capture_result.failure_class.clone(),
        };

        if has_env_contract_fail {
            final_status = ValidationRunStatus::ContractFail;
            final_reason = Some("environment_policy_contract_fail".to_string());
            command_entries.push(entry);
            break;
        }

        if has_env_failure {
            final_status = ValidationRunStatus::Fail;
            final_reason = Some(format!(
                "environment_policy_failed: {}",
                validation_command.id
            ));
            command_entries.push(entry);
            break;
        }

        if capture_result.status == CommandStatus::Fail {
            if validation_command.required {
                final_status = ValidationRunStatus::Fail;
                final_reason = Some(format!(
                    "required command failed: {}",
                    validation_command.id
                ));
                command_entries.push(entry);
                break;
            }
            has_optional_failure = true;
        }

        command_entries.push(entry);
    }

    if !matches!(final_status, ValidationRunStatus::Fail) {
        final_status = if has_env_contract_fail {
            ValidationRunStatus::ContractFail
        } else if has_env_failure {
            ValidationRunStatus::Fail
        } else if has_optional_failure || has_env_warning {
            ValidationRunStatus::PassWithWarnings
        } else {
            ValidationRunStatus::Pass
        };
    }

    let manifest = ValidationRunManifest {
        schema_version: 1,
        validation_run_id,
        contract_path: contract_path_string,
        contract_sha256,
        repo_path: repo_path_string,
        status: final_status,
        started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        commands: command_entries,
        github_ci_used_as_evidence: false,
        environment_policy: environment_policy_summary,
        reason: final_reason,
    };
    write_manifest(&manifest_path, &manifest)?;

    Ok(ValidationRunOutcome {
        manifest,
        manifest_path: repo_relative_string(&repo_root, &manifest_path),
    })
}

fn build_capture_request(
    repo: &Path,
    command: &ValidationCommand,
    environment_policy: &EnvironmentPolicy,
) -> CaptureRequest {
    CaptureRequest {
        id: command.id.clone(),
        repo: repo.to_path_buf(),
        command: crate::evidence::CommandSpec {
            program: command.program.clone(),
            args: command.args.clone(),
        },
        policy: CapturePolicy {
            wall_timeout_seconds: command.wall_timeout_seconds,
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            max_combined_output_bytes: 20 * 1024 * 1024,
            on_output_limit: OutputLimitPolicy::FailAndTerminate,
            capture_env: true,
        },
        environment_policy: Some(environment_policy.clone()),
    }
}

fn aggregate_environment_policy_status(
    current: &EnvironmentPolicyStatus,
    next: &EnvironmentPolicyStatus,
) -> EnvironmentPolicyStatus {
    use EnvironmentPolicyStatus::*;
    match (current, next) {
        (ContractFail, _) | (_, ContractFail) => ContractFail,
        (Fail, _) | (_, Fail) => Fail,
        (Warn, _) | (_, Warn) => Warn,
        _ => Pass,
    }
}

fn write_manifest(
    path: &Path,
    manifest: &ValidationRunManifest,
) -> Result<(), ValidationRunnerError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn generate_validation_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("validation-{}-{}", now, std::process::id())
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
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn repo_dir() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ccl_validation_repo_{}_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(dir.join(".git")).unwrap();
        dir
    }

    fn helper_exe() -> PathBuf {
        static HELPER: OnceLock<PathBuf> = OnceLock::new();
        HELPER
            .get_or_init(|| {
                let root = std::env::temp_dir().join(format!(
                    "ccl_validation_helper_{}_{}",
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
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "pass" => {
            println!("pass");
        }
        "fail" => {
            eprintln!("fail");
            std::process::exit(7);
        }
        "optional" => {
            eprintln!("optional fail");
            std::process::exit(3);
        }
        "sleep" => {
            thread::sleep(Duration::from_secs(5));
        }
        _ => {
            println!("unknown");
        }
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

    fn contract_file(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join("contract.json");
        fs::write(&path, content).unwrap();
        path
    }

    fn json_escape_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }

    #[test]
    fn validation_run_passes_with_required_command() {
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-pass",
        "program": "{}",
        "args": ["pass"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper
            ),
        );

        let outcome = run_validation(&contract, &repo).unwrap();
        assert_eq!(outcome.manifest.status, ValidationRunStatus::Pass);
        assert!(Path::new(&repo.join(outcome.manifest_path)).exists());
        assert_eq!(outcome.manifest.commands.len(), 1);
        assert!(outcome.manifest.commands[0]
            .result_path
            .starts_with(".ccl/runs/"));
    }

    #[test]
    fn validation_run_fails_on_required_command_failure() {
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-fail",
        "program": "{}",
        "args": ["fail"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper
            ),
        );

        let outcome = run_validation(&contract, &repo).unwrap();
        assert_eq!(outcome.manifest.status, ValidationRunStatus::Fail);
        assert_eq!(outcome.manifest.commands.len(), 1);
        assert_eq!(outcome.manifest.commands[0].status, CommandStatus::Fail);
    }

    #[test]
    fn validation_run_contract_fails_without_commands() {
        let repo = repo_dir();
        let contract = contract_file(
            &repo,
            r#"{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture",
  "required_context": {
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  },
  "allowed_paths": ["crates/ccl-core/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}"#,
        );

        let outcome = run_validation(&contract, &repo).unwrap();
        assert_eq!(outcome.manifest.status, ValidationRunStatus::ContractFail);
        assert_eq!(
            outcome.manifest.reason.as_deref(),
            Some("no_validation_commands")
        );
    }

    #[test]
    fn validation_run_manifest_records_contract_hash_and_ci_flag() {
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-pass",
        "program": "{}",
        "args": ["pass"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper
            ),
        );

        let contract_bytes = fs::read(&contract).unwrap();
        let expected_sha = sha256_hex(&contract_bytes);
        let outcome = run_validation(&contract, &repo).unwrap();
        assert_eq!(outcome.manifest.contract_sha256, expected_sha);
        assert!(!outcome.manifest.github_ci_used_as_evidence);
    }

    #[test]
    fn optional_command_failure_turns_into_warnings() {
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-pass",
        "program": "{}",
        "args": ["pass"],
        "required": true,
        "wall_timeout_seconds": 300
      }},
      {{
        "id": "helper-optional-fail",
        "program": "{}",
        "args": ["optional"],
        "required": false,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper, helper
            ),
        );

        let outcome = run_validation(&contract, &repo).unwrap();
        assert_eq!(
            outcome.manifest.status,
            ValidationRunStatus::PassWithWarnings
        );
        assert_eq!(outcome.manifest.commands.len(), 2);
        assert!(!outcome.manifest.commands[1].required);
    }

    #[test]
    fn environment_policy_warn_turns_validation_into_pass_with_warnings() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture with environment policy",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-pass",
        "program": "{}",
        "args": ["pass"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "environment_policy": {{
    "mode": "warn",
    "deny_prefixes": ["CCL_TEST_DENY_"],
    "redact_patterns": ["TOKEN"],
    "unknown": "warn"
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper
            ),
        );

        std::env::set_var("CCL_TEST_DENY_WARN", "1");
        let outcome = run_validation(&contract, &repo).unwrap();
        std::env::remove_var("CCL_TEST_DENY_WARN");

        assert_eq!(
            outcome.manifest.status,
            ValidationRunStatus::PassWithWarnings
        );
        assert_eq!(
            outcome.manifest.environment_policy.status,
            EnvironmentPolicyStatus::Warn
        );
        assert!(outcome.manifest.environment_policy.warnings_count > 0);
    }

    #[test]
    fn environment_policy_enforce_fails_before_command_runs() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let repo = repo_dir();
        let helper = helper_exe();
        let helper = json_escape_path(&helper);
        let contract = contract_file(
            &repo,
            &format!(
                r#"{{
  "project": "CCL",
  "workstream": "Validation",
  "task_type": "guard_gate",
  "objective": "Validate commands through capture with environment policy",
  "required_context": {{
    "dna": true,
    "latest_prs": 10,
    "project_ledger": true
  }},
  "allowed_paths": ["crates/ccl-core/**", "crates/ccl-cli/**", "ledger/**", "examples/**", "ci/**"],
  "forbidden_paths": [".github/**"],
  "required_validation": ["command validation"],
  "validation": {{
    "commands": [
      {{
        "id": "helper-pass",
        "program": "{}",
        "args": ["pass"],
        "required": true,
        "wall_timeout_seconds": 300
      }}
    ]
  }},
  "environment_policy": {{
    "mode": "enforce",
    "deny_prefixes": ["CCL_TEST_DENY_"],
    "redact_patterns": ["TOKEN"],
    "unknown": "fail"
  }},
  "github_ci_as_evidence": false,
  "ledger_update_required": true
}}"#,
                helper
            ),
        );

        std::env::set_var("CCL_TEST_DENY_FAIL", "1");
        let outcome = run_validation(&contract, &repo).unwrap();
        std::env::remove_var("CCL_TEST_DENY_FAIL");

        assert_eq!(outcome.manifest.status, ValidationRunStatus::Fail);
        assert_eq!(
            outcome.manifest.environment_policy.status,
            EnvironmentPolicyStatus::Fail
        );
        assert_eq!(
            outcome.manifest.commands[0].failure_class,
            Some(FailureClass::EnvironmentPolicyFailed)
        );
    }
}
