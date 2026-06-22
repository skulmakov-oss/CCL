use ccl_core::admission;
use ccl_core::capture::{capture_command, CaptureError};
use ccl_core::evidence::{CapturePolicy, CaptureRequest, CommandSpec};
use ccl_core::gate::{self, GateRunRequest};
use ccl_core::ledger as ledger_core;
use ccl_core::preflight;
use ccl_core::release;
use ccl_core::scope::{self, ScopeCheckStatus};
use ccl_core::task_contract::TaskContract;
use ccl_core::validation_runner::{self, ValidationRunManifest, ValidationRunStatus};
use ccl_core::verdict::{AdmissionStatus, VerdictStatus};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "ccl")]
#[command(version)]
#[command(about = "CCL - Cerebral Control Layer", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check a task contract
    Contract {
        #[command(subcommand)]
        action: ContractCommands,
    },
    /// Run repository preflight
    Preflight {
        #[arg(long)]
        repo: PathBuf,
    },
    /// Capture command evidence
    Capture {
        #[arg(long)]
        id: String,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value_t = 300)]
        wall_timeout: u64,
        #[arg(long, default_value_t = 10 * 1024 * 1024)]
        max_stdout_bytes: u64,
        #[arg(long, default_value_t = 10 * 1024 * 1024)]
        max_stderr_bytes: u64,
        #[arg(long, default_value_t = 20 * 1024 * 1024)]
        max_combined_output_bytes: u64,
        #[arg(value_name = "COMMAND", required = true, trailing_var_arg = true, num_args = 1..)]
        command: Vec<String>,
    },
    /// Run contract-bound validation
    Validate {
        #[command(subcommand)]
        action: ValidateCommands,
    },
    /// Run scope/diff policy checking
    Scope {
        #[command(subcommand)]
        action: ScopeCommands,
    },
    /// Compute admission verdict from existing evidence
    Admission {
        #[command(subcommand)]
        action: AdmissionCommands,
    },
    /// Verify ledger semantics from existing evidence
    Ledger {
        #[command(subcommand)]
        action: LedgerCommands,
    },
    /// Run a local release dry-run
    Release {
        #[command(subcommand)]
        action: ReleaseCommands,
    },
    /// Run the full gate orchestration
    Gate {
        #[command(subcommand)]
        action: GateCommands,
    },
}

#[derive(Subcommand)]
enum ContractCommands {
    Check { path: PathBuf },
}

#[derive(Subcommand)]
enum ValidateCommands {
    Run {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum ScopeCommands {
    Check {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum AdmissionCommands {
    Verdict {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        validation_manifest: PathBuf,
        #[arg(long)]
        scope_manifest: PathBuf,
        #[arg(long, default_value = "ledger/project-ledger.md")]
        ledger: PathBuf,
    },
}

#[derive(Subcommand)]
enum LedgerCommands {
    Verify {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "ledger/project-ledger.md")]
        ledger: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReleaseCommands {
    DryRun {
        #[arg(long)]
        version: String,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "examples/ccl-admission-task-contract.json")]
        contract: PathBuf,
    },
}

#[derive(Subcommand)]
enum GateCommands {
    Run {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        verbose: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Contract { action }) => match action {
            ContractCommands::Check { path } => {
                let file_content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to read contract file: {}", e);
                        std::process::exit(1);
                    }
                };

                let contract = match TaskContract::from_json(&file_content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to parse contract JSON: {}", e);
                        std::process::exit(1);
                    }
                };

                let report = contract.validate();

                println!("Contract: {}", path.display());
                println!("Project: {}", contract.project);
                println!("Workstream: {}", contract.workstream);

                let type_str = contract.type_as_string();
                println!("Task type: {}", type_str);

                println!("Status: {}", report.status);

                if !report.errors.is_empty() {
                    println!("\nErrors:");
                    for err in &report.errors {
                        println!("- {}", err.0);
                    }
                }

                if !report.warnings.is_empty() {
                    println!("\nWarnings:");
                    for warn in &report.warnings {
                        println!("- {}", warn.0);
                    }
                }

                if report.status == VerdictStatus::Fail {
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::Preflight { repo }) => {
            let report = preflight::run_preflight(repo);
            println!("Repository preflight");
            println!("Path: {}", report.repo_path);
            println!(".git: {}", if report.has_git { "YES" } else { "NO" });
            println!(
                "README.md: {}",
                if report.has_readme { "YES" } else { "NO" }
            );
            println!("docs/: {}", if report.has_docs { "YES" } else { "NO" });
            println!(
                "examples/: {}",
                if report.has_examples { "YES" } else { "NO" }
            );
            println!(
                "Cargo.toml: {}",
                if report.has_cargo_toml { "YES" } else { "NO" }
            );
            println!("Status: {}", report.verdict.status);

            if report.verdict.status == VerdictStatus::Fail {
                std::process::exit(1);
            }
        }
        Some(Commands::Capture {
            id,
            repo,
            wall_timeout,
            max_stdout_bytes,
            max_stderr_bytes,
            max_combined_output_bytes,
            command,
        }) => {
            let policy = CapturePolicy {
                wall_timeout_seconds: *wall_timeout,
                max_stdout_bytes: *max_stdout_bytes,
                max_stderr_bytes: *max_stderr_bytes,
                max_combined_output_bytes: *max_combined_output_bytes,
                ..CapturePolicy::default()
            };

            if command.is_empty() {
                eprintln!("Capture requires a command after --");
                std::process::exit(1);
            }

            let capture_request = CaptureRequest {
                id: id.clone(),
                repo: repo.clone(),
                command: CommandSpec {
                    program: command[0].clone(),
                    args: command[1..].to_vec(),
                },
                policy,
                environment_policy: None,
            };

            match capture_command(capture_request) {
                Ok(outcome) => {
                    println!("Capture run: {}", outcome.run.run_id);
                    println!("Command: {}", outcome.command_result.program);
                    println!("Status: {}", outcome.command_result.status);
                    println!("Result: {}", outcome.command_result.result_path);
                    println!("Evidence manifest: {}", outcome.run.evidence_manifest_path);
                    if outcome.command_result.status == ccl_core::evidence::CommandStatus::Fail {
                        std::process::exit(1);
                    }
                }
                Err(err) => {
                    report_capture_error(err);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Validate { action }) => match action {
            ValidateCommands::Run { contract, repo } => {
                match validation_runner::run_validation(contract, repo) {
                    Ok(outcome) => {
                        print_validation_run(
                            &outcome.manifest,
                            contract,
                            repo,
                            &outcome.manifest_path,
                        );
                        std::process::exit(validation_exit_code(&outcome.manifest.status));
                    }
                    Err(err) => {
                        eprintln!("Validation runner error: {}", err);
                        std::process::exit(40);
                    }
                }
            }
        },
        Some(Commands::Scope { action }) => match action {
            ScopeCommands::Check { contract, repo } => match scope::run_scope_check(contract, repo)
            {
                Ok(outcome) => {
                    print_scope_check(&outcome.manifest, contract, repo, &outcome.manifest_path);
                    std::process::exit(scope_exit_code(&outcome.manifest.status));
                }
                Err(err) => {
                    eprintln!("Scope checker error: {}", err);
                    std::process::exit(40);
                }
            },
        },
        Some(Commands::Admission { action }) => match action {
            AdmissionCommands::Verdict {
                contract,
                repo,
                validation_manifest: _validation_manifest,
                scope_manifest: _scope_manifest,
                ledger: _ledger,
            } => match admission::run_admission_verdict(admission::AdmissionVerdictRequest {
                contract_path: contract.clone(),
                repo: repo.clone(),
                validation_manifest_path: _validation_manifest.clone(),
                scope_manifest_path: _scope_manifest.clone(),
                ledger_path: _ledger.clone(),
            }) {
                Ok(outcome) => {
                    print_admission_verdict(
                        &outcome.manifest,
                        contract,
                        repo,
                        &outcome.manifest_path,
                    );
                    std::process::exit(admission_exit_code(&outcome.manifest.status));
                }
                Err(err) => {
                    eprintln!("Admission verdict error: {}", err);
                    std::process::exit(40);
                }
            },
        },
        Some(Commands::Ledger { action }) => match action {
            LedgerCommands::Verify {
                contract,
                repo,
                ledger,
            } => {
                match ledger_core::run_ledger_verification(ledger_core::LedgerVerificationRequest {
                    contract_path: contract.clone(),
                    repo: repo.clone(),
                    ledger_path: ledger.clone(),
                }) {
                    Ok(outcome) => {
                        print_ledger_verification(
                            &outcome.manifest,
                            contract,
                            repo,
                            ledger,
                            &outcome.manifest_path,
                        );
                        std::process::exit(ledger_exit_code(&outcome.manifest.status));
                    }
                    Err(err) => {
                        eprintln!("Ledger verification error: {}", err);
                        std::process::exit(40);
                    }
                }
            }
        },
        Some(Commands::Release { action }) => match action {
            ReleaseCommands::DryRun {
                version,
                repo,
                contract,
            } => match release::run_release_dry_run(release::ReleaseDryRunRequest {
                repo: repo.clone(),
                version: version.clone(),
                contract_path: contract.clone(),
            }) {
                Ok(outcome) => {
                    print_release_dry_run(&outcome, version, repo);
                    std::process::exit(release_exit_code(&outcome.status));
                }
                Err(err) => {
                    eprintln!("Release dry-run error: {}", err);
                    std::process::exit(40);
                }
            },
        },
        Some(Commands::Gate { action }) => match action {
            GateCommands::Run {
                contract,
                repo,
                verbose,
            } => {
                match gate::run_gate(GateRunRequest {
                    contract_path: contract.clone(),
                    repo: repo.clone(),
                }) {
                    Ok(outcome) => {
                        print_gate_run(
                            &outcome.manifest,
                            contract,
                            repo,
                            &outcome.manifest_path,
                            *verbose,
                        );
                        std::process::exit(admission_exit_code(&outcome.manifest.status));
                    }
                    Err(err) => {
                        eprintln!("Gate orchestration error: {}", err);
                        std::process::exit(40);
                    }
                }
            }
        },
        None => {}
    }
}

fn report_capture_error(err: CaptureError) {
    match err {
        CaptureError::InvalidCommand(message) => eprintln!("Capture command error: {}", message),
        CaptureError::Io(error) => eprintln!("Capture I/O error: {}", error),
        CaptureError::Json(error) => eprintln!("Capture JSON error: {}", error),
        CaptureError::SpawnFailed(message) => eprintln!("Capture spawn error: {}", message),
    }
}

fn print_validation_run(
    manifest: &validation_runner::ValidationRunManifest,
    contract: &Path,
    repo: &Path,
    manifest_path: &str,
) {
    println!("CCL validation run");
    println!("Contract: {}", contract.display());
    println!("Repo: {}", repo.display());
    println!("Status: {}", manifest.status);
    if let Some(reason) = &manifest.reason {
        println!("Reason: {}", reason);
    }
    println!();
    println!("Commands:");
    for command in &manifest.commands {
        println!("- {}: {}", command.id, command.status);
    }
    if let Some(failed_required) = manifest.commands.iter().find(|command| {
        command.required && command.status == ccl_core::evidence::CommandStatus::Fail
    }) {
        println!();
        println!("Failed required command:");
        println!("{}", failed_required.id);
        println!();
        println!("Evidence:");
        println!("{}", failed_required.result_path);
    }
    if manifest.environment_policy.checked {
        println!();
        println!("Environment policy:");
        println!("- mode: {}", manifest.environment_policy.mode);
        println!("- status: {}", manifest.environment_policy.status);
        println!("- warnings: {}", manifest.environment_policy.warnings_count);
        println!(
            "- violations: {}",
            manifest.environment_policy.violations_count
        );
    }
    println!();
    println!("Manifest:");
    println!("{}", manifest_path);
    println!();
    println!("GitHub CI used as evidence: NO");
    println!("Agent testimony used as evidence: NO");
    println!("Verdict source: captured local evidence");
}

fn validation_exit_code(status: &ValidationRunStatus) -> i32 {
    match status {
        ValidationRunStatus::Pass => 0,
        ValidationRunStatus::PassWithWarnings => 10,
        ValidationRunStatus::Fail => 20,
        ValidationRunStatus::ContractFail => 30,
    }
}

fn print_scope_check(
    manifest: &scope::ScopeCheckManifest,
    contract: &Path,
    repo: &Path,
    manifest_path: &str,
) {
    println!("CCL scope check");
    println!("Contract: {}", contract.display());
    println!("Repo: {}", repo.display());
    println!("Status: {}", manifest.status);
    println!();
    println!("Summary:");
    println!("- changed files: {}", manifest.summary.changed_files_count);
    println!(
        "- untracked files: {}",
        manifest.summary.untracked_files_count
    );
    println!(
        "- diff lines: {} / {}",
        manifest.summary.diff_total_lines, manifest.summary.max_diff_lines
    );
    if !manifest.violations.is_empty() {
        println!();
        println!("Violations:");
        for violation in &manifest.violations {
            println!("- {}: {}", violation.kind, violation.path);
        }
    }
    println!();
    println!("Manifest:");
    println!("{}", manifest_path);
    println!();
    println!("GitHub CI used as evidence: NO");
}

fn scope_exit_code(status: &ScopeCheckStatus) -> i32 {
    match status {
        ScopeCheckStatus::Pass => 0,
        ScopeCheckStatus::PassWithWarnings => 10,
        ScopeCheckStatus::Fail => 20,
        ScopeCheckStatus::ContractFail => 30,
    }
}

fn print_admission_verdict(
    manifest: &admission::AdmissionVerdictManifest,
    contract: &Path,
    repo: &Path,
    manifest_path: &str,
) {
    println!("CCL admission verdict");
    println!("Contract: {}", contract.display());
    println!("Repo: {}", repo.display());
    println!("Status: {}", manifest.status);
    println!();
    println!("Evidence:");
    println!("- validation: {}", manifest.evidence.validation_status);
    println!("- scope: {}", manifest.evidence.scope_status);
    println!(
        "- ledger exists: {}",
        if manifest.evidence.ledger_exists {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- GitHub CI used as evidence: {}",
        if manifest.github_ci_used_as_evidence {
            "YES"
        } else {
            "NO"
        }
    );
    println!();
    if !manifest.warnings.is_empty() {
        println!("Warnings:");
        for warning in &manifest.warnings {
            println!("- {}: {}", warning.kind, warning.reason);
        }
        println!();
    }
    if !manifest.violations.is_empty() {
        println!("Violations:");
        for violation in &manifest.violations {
            println!("- {}: {}", violation.kind, violation.reason);
        }
        println!();
    }
    println!("Manifest:");
    println!("{}", manifest_path);
}

fn print_gate_run(
    manifest: &gate::GateRunManifest,
    contract: &Path,
    repo: &Path,
    manifest_path: &str,
    verbose: bool,
) {
    let validation_manifest_path =
        gate_step_manifest_path(manifest, gate::GateStepName::Validation)
            .map(|path| path.to_string());
    let scope_manifest_path =
        gate_step_manifest_path(manifest, gate::GateStepName::Scope).map(|path| path.to_string());
    let admission_manifest_path = gate_step_manifest_path(manifest, gate::GateStepName::Admission)
        .map(|path| path.to_string());

    let validation_manifest = validation_manifest_path
        .as_deref()
        .and_then(|path| load_validation_manifest(repo, path));
    let admission_manifest = admission_manifest_path
        .as_deref()
        .and_then(|path| load_admission_manifest(repo, path));

    let environment_policy = validation_manifest
        .as_ref()
        .map(|manifest| &manifest.environment_policy);

    let ledger_status = admission_manifest
        .as_ref()
        .and_then(|manifest| manifest.evidence.ledger_verification_status.clone())
        .unwrap_or_else(|| "N/A".to_string());
    let ledger_manifest_path = admission_manifest
        .as_ref()
        .and_then(|manifest| manifest.evidence.ledger_verification_manifest_path.clone())
        .unwrap_or_else(|| "N/A".to_string());

    println!("CCL Gate Summary");
    println!("================");
    println!();
    println!("Status: {}", manifest.status);
    println!("Contract: {}", contract.display());
    println!();
    println!("Repository: {}", repo.display());
    println!();
    println!("Layers:");
    println!(
        "- validation: {}",
        gate_step_status(manifest, gate::GateStepName::Validation)
    );
    println!(
        "- scope: {}",
        gate_step_status(manifest, gate::GateStepName::Scope)
    );
    println!("- ledger: {}", ledger_status);
    println!(
        "- environment: {}",
        environment_policy
            .map(|policy| policy.status.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    );
    println!("- admission: {}", manifest.status);
    println!();
    println!("Counts:");
    println!("- warnings: {}", manifest.warnings.len());
    println!("- violations: {}", manifest.violations.len());
    if let Some(policy) = environment_policy {
        println!("- environment warnings: {}", policy.warnings_count);
        println!("- environment violations: {}", policy.violations_count);
    }
    if let Some(policy) = environment_policy {
        println!();
        println!("Environment policy:");
        println!("- mode: {}", policy.mode);
        println!("- status: {}", policy.status);
        println!("- warnings: {}", policy.warnings_count);
        println!("- violations: {}", policy.violations_count);
    }
    println!();
    println!("Artifacts:");
    println!("- gate manifest: {}", manifest_path);
    if let Some(path) = validation_manifest_path.as_deref() {
        println!("- validation manifest: {}", path);
    }
    if let Some(path) = scope_manifest_path.as_deref() {
        println!("- scope manifest: {}", path);
    }
    if let Some(path) = admission_manifest_path.as_deref() {
        println!("- admission verdict: {}", path);
    }
    println!("- ledger manifest: {}", ledger_manifest_path);
    if !manifest.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &manifest.warnings {
            println!("- {}: {}", warning.kind, warning.reason);
        }
    }
    if !manifest.violations.is_empty() {
        println!();
        println!("Violations:");
        for violation in &manifest.violations {
            println!("- {}: {}", violation.kind, violation.reason);
        }
    }
    if verbose {
        println!();
        println!("Verbose:");
        println!("- gate run id: {}", manifest.gate_run_id);
        println!("- contract sha256: {}", manifest.contract_sha256);
        println!("- started_unix_ms: {}", manifest.started_unix_ms);
        println!("- finished_unix_ms: {}", manifest.finished_unix_ms);
        if let Some(validation) = validation_manifest {
            println!("- validation commands: {}", validation.commands.len());
            let required_failures = validation
                .commands
                .iter()
                .filter(|command| {
                    command.required && command.status == ccl_core::evidence::CommandStatus::Fail
                })
                .count();
            println!("- validation required failures: {}", required_failures);
            println!(
                "- validation environment policy: {} / {}",
                validation.environment_policy.mode, validation.environment_policy.status
            );
        }
        if let Some(admission) = admission_manifest {
            println!(
                "- admission evidence: validation={}, scope={}, ledger={}",
                admission.evidence.validation_status,
                admission.evidence.scope_status,
                admission
                    .evidence
                    .ledger_verification_status
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!("- admission warnings: {}", admission.warnings.len());
            println!("- admission violations: {}", admission.violations.len());
        }
    }
    println!();
    println!("Manifest:");
    println!("{}", manifest_path);
    println!();
    println!("GitHub CI used as evidence: NO");
}

fn print_release_dry_run(outcome: &release::ReleaseDryRunOutcome, version: &str, repo: &Path) {
    println!("CCL Release Dry-Run Summary");
    println!("============================");
    println!();
    println!("Status: {}", outcome.status);
    println!("Version: {}", version);
    println!("Tag: {}", outcome.manifest.tag);
    println!("Repo: {}", repo.display());
    println!("Gate status: {}", outcome.manifest.gate.gate_status);
    println!("Tree clean: {}", outcome.manifest.source.tree_clean);
    println!(
        "Schema present: {}",
        outcome.manifest.schema.schema_file_present
    );
    println!(
        "Schema JSON valid: {}",
        outcome.manifest.schema.schema_json_valid
    );
    println!("Manifest: {}", outcome.manifest_path);
    println!();
    println!("Dry-run only:");
    println!(
        "- tag created: {}",
        if outcome.manifest.policy.tag_created {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- artifacts created: {}",
        if outcome.manifest.policy.artifacts_created {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- checksums generated: {}",
        if outcome.manifest.policy.checksums_generated {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- GitHub Release created: {}",
        if outcome.manifest.policy.github_release_created {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- crates.io publish: {}",
        if outcome.manifest.policy.crates_io_published {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- GitHub CI used as evidence: {}",
        if outcome.manifest.policy.github_ci_used_as_evidence {
            "YES"
        } else {
            "NO"
        }
    );
    println!();
    println!("Local CCL gate requirement:");
    println!(
        "- release entry required for real release: {}",
        if outcome
            .manifest
            .ledger
            .release_entry_required_for_real_release
        {
            "YES"
        } else {
            "NO"
        }
    );
    println!(
        "- dry-run entry recorded: {}",
        if outcome.manifest.ledger.dry_run_entry_recorded {
            "YES"
        } else {
            "NO"
        }
    );
    if !outcome.manifest.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &outcome.manifest.warnings {
            println!("- {}", warning);
        }
    }
    if !outcome.manifest.violations.is_empty() {
        println!();
        println!("Violations:");
        for violation in &outcome.manifest.violations {
            println!("- {}", violation);
        }
    }
}

fn print_ledger_verification(
    manifest: &ledger_core::LedgerVerificationManifest,
    contract: &Path,
    repo: &Path,
    ledger_path: &Path,
    manifest_path: &str,
) {
    println!("CCL ledger verify");
    println!("Contract: {}", contract.display());
    println!("Repo: {}", repo.display());
    println!("Ledger: {}", ledger_path.display());
    println!("Status: {}", manifest.status);
    if let Some(reason) = &manifest.reason {
        println!("Reason: {}", reason);
    }
    if let Some(entry) = &manifest.matched_entry {
        println!();
        println!("Matched entry:");
        println!("{}", entry.heading);
    }
    if !manifest.checks.is_empty() {
        println!();
        println!("Checks:");
        for check in &manifest.checks {
            println!("- {}: {}", check.kind, check.status);
        }
    }
    if !manifest.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &manifest.warnings {
            println!("- {}: {}", warning.kind, warning.reason);
        }
    }
    if !manifest.violations.is_empty() {
        println!();
        println!("Violations:");
        for violation in &manifest.violations {
            println!("- {}: {}", violation.kind, violation.reason);
        }
    }
    println!();
    println!("Manifest:");
    println!("{}", manifest_path);
    println!();
    println!("GitHub CI used as evidence: NO");
}

fn release_exit_code(status: &release::ReleaseDryRunStatus) -> i32 {
    match status {
        release::ReleaseDryRunStatus::Pass => 0,
        release::ReleaseDryRunStatus::PassWithWarnings => 10,
        release::ReleaseDryRunStatus::Fail => 20,
        release::ReleaseDryRunStatus::ContractFail => 30,
    }
}

fn admission_exit_code(status: &AdmissionStatus) -> i32 {
    match status {
        AdmissionStatus::Pass => 0,
        AdmissionStatus::PassWithWarnings => 10,
        AdmissionStatus::Fail => 20,
        AdmissionStatus::ContractFail => 30,
        AdmissionStatus::InternalError => 40,
    }
}

fn ledger_exit_code(status: &ledger_core::LedgerVerificationStatus) -> i32 {
    match status {
        ledger_core::LedgerVerificationStatus::Pass => 0,
        ledger_core::LedgerVerificationStatus::PassWithWarnings => 10,
        ledger_core::LedgerVerificationStatus::Fail => 20,
        ledger_core::LedgerVerificationStatus::ContractFail => 30,
    }
}

fn gate_step_manifest_path(
    manifest: &gate::GateRunManifest,
    name: gate::GateStepName,
) -> Option<&str> {
    manifest
        .steps
        .iter()
        .find(|step| step.name == name)
        .map(|step| step.manifest_path.as_str())
}

fn gate_step_status(manifest: &gate::GateRunManifest, name: gate::GateStepName) -> String {
    manifest
        .steps
        .iter()
        .find(|step| step.name == name)
        .map(|step| step.status.clone())
        .unwrap_or_else(|| "N/A".to_string())
}

fn load_validation_manifest(repo: &Path, path: &str) -> Option<ValidationRunManifest> {
    fs::read_to_string(resolve_manifest_path(repo, path))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn load_admission_manifest(
    repo: &Path,
    path: &str,
) -> Option<ccl_core::admission::AdmissionVerdictManifest> {
    fs::read_to_string(resolve_manifest_path(repo, path))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn resolve_manifest_path(repo: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        repo.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccl_core::admission::{
        AdmissionEvidenceSummary, AdmissionVerdictManifest, AdmissionWarning,
    };
    use ccl_core::environment::{EnvironmentPolicyMode, EnvironmentPolicyStatus};
    use ccl_core::gate::{GateRunManifest, GateStepManifest, GateStepName};
    use ccl_core::validation_runner::{
        ValidationEnvironmentPolicyCommandResult, ValidationEnvironmentPolicySummary,
        ValidationRunManifest, ValidationRunStatus,
    };
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "{}_{}_{}_{}",
            prefix,
            std::process::id(),
            system_time_ms(SystemTime::now()),
            unique
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn system_time_ms(time: SystemTime) -> u128 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }

    #[test]
    fn gate_summary_helpers_extract_manifest_paths_and_statuses() {
        let manifest = GateRunManifest {
            schema_version: 1,
            gate_run_id: "gate-1".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "abc".to_string(),
            repo_path: ".".to_string(),
            status: AdmissionStatus::Pass,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            github_ci_used_as_evidence: false,
            steps: vec![
                GateStepManifest {
                    name: GateStepName::Validation,
                    status: "PASS".to_string(),
                    manifest_path: ".ccl/runs/validation-1/validation-run-manifest.json"
                        .to_string(),
                },
                GateStepManifest {
                    name: GateStepName::Scope,
                    status: "PASS".to_string(),
                    manifest_path: ".ccl/runs/scope-1/scope-check-manifest.json".to_string(),
                },
                GateStepManifest {
                    name: GateStepName::Admission,
                    status: "PASS".to_string(),
                    manifest_path: ".ccl/runs/admission-1/admission-verdict.json".to_string(),
                },
            ],
            warnings: vec![],
            violations: vec![],
        };

        assert_eq!(
            gate_step_status(&manifest, GateStepName::Validation),
            "PASS"
        );
        assert_eq!(gate_step_status(&manifest, GateStepName::Scope), "PASS");
        assert_eq!(
            gate_step_manifest_path(&manifest, GateStepName::Admission),
            Some(".ccl/runs/admission-1/admission-verdict.json")
        );
    }

    #[test]
    fn manifest_loaders_round_trip_gate_reporting_inputs() {
        let dir = temp_dir("ccl_gate_summary");
        let validation_path = dir.join("validation.json");
        let admission_path = dir.join("admission.json");

        let validation = ValidationRunManifest {
            schema_version: 1,
            validation_run_id: "validation-1".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "abc".to_string(),
            repo_path: ".".to_string(),
            status: ValidationRunStatus::PassWithWarnings,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            commands: vec![],
            github_ci_used_as_evidence: false,
            environment_policy: ValidationEnvironmentPolicySummary {
                mode: EnvironmentPolicyMode::Warn,
                status: EnvironmentPolicyStatus::Warn,
                checked: true,
                command_results: vec![ValidationEnvironmentPolicyCommandResult {
                    command_id: "cargo-version".to_string(),
                    status: EnvironmentPolicyStatus::Warn,
                    warnings_count: 1,
                    violations_count: 0,
                    redacted_variables_count: 0,
                }],
                warnings_count: 1,
                violations_count: 0,
                redacted_variables_count: 0,
            },
            reason: None,
        };

        let admission = AdmissionVerdictManifest {
            schema_version: 1,
            admission_run_id: "admission-1".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "abc".to_string(),
            repo_path: ".".to_string(),
            validation_manifest_path: validation_path.to_string_lossy().into_owned(),
            scope_manifest_path: ".ccl/runs/scope-1/scope-check-manifest.json".to_string(),
            ledger_path: "ledger/project-ledger.md".to_string(),
            status: AdmissionStatus::PassWithWarnings,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            github_ci_used_as_evidence: false,
            evidence: AdmissionEvidenceSummary {
                validation_status: "PASS WITH WARNINGS".to_string(),
                scope_status: "PASS".to_string(),
                validation_github_ci_used_as_evidence: false,
                scope_github_ci_used_as_evidence: false,
                scope_violations_count: 0,
                validation_commands_count: 1,
                required_validation_failures_count: 0,
                missing_command_result_artifacts_count: 0,
                ledger_verification_status: Some("PASS".to_string()),
                ledger_verification_manifest_path: Some(
                    ".ccl/runs/ledger-1/ledger-verification-manifest.json".to_string(),
                ),
                ledger_exists: true,
                ledger_update_required: true,
                contract_sha256_matches_validation: true,
                contract_sha256_matches_scope: true,
            },
            violations: vec![],
            warnings: vec![AdmissionWarning {
                kind: "validation_environment_policy_warned".to_string(),
                reason: "validation manifest reported environment policy WARN".to_string(),
            }],
            decision_rule: "validation PASS + scope PASS + no hard violations".to_string(),
            reason: None,
        };

        fs::write(
            &validation_path,
            serde_json::to_vec_pretty(&validation).unwrap(),
        )
        .unwrap();
        fs::write(
            &admission_path,
            serde_json::to_vec_pretty(&admission).unwrap(),
        )
        .unwrap();

        let loaded_validation = load_validation_manifest(&dir, "validation.json")
            .expect("validation manifest should load");
        let loaded_admission =
            load_admission_manifest(&dir, "admission.json").expect("admission should load");

        assert_eq!(
            loaded_validation.environment_policy.status,
            EnvironmentPolicyStatus::Warn
        );
        assert_eq!(
            loaded_admission
                .evidence
                .ledger_verification_status
                .as_deref(),
            Some("PASS")
        );
        assert_eq!(
            loaded_admission
                .evidence
                .ledger_verification_manifest_path
                .as_deref(),
            Some(".ccl/runs/ledger-1/ledger-verification-manifest.json")
        );
    }

    #[test]
    fn manifest_loaders_resolve_repo_relative_paths() {
        let repo = temp_dir("ccl_gate_summary_repo");
        let validation_dir = repo.join(".ccl").join("runs").join("validation-1");
        let admission_dir = repo.join(".ccl").join("runs").join("admission-1");
        fs::create_dir_all(&validation_dir).unwrap();
        fs::create_dir_all(&admission_dir).unwrap();

        let validation = ValidationRunManifest {
            schema_version: 1,
            validation_run_id: "validation-1".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "abc".to_string(),
            repo_path: repo.to_string_lossy().into_owned(),
            status: ValidationRunStatus::Pass,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            commands: vec![],
            github_ci_used_as_evidence: false,
            environment_policy: ValidationEnvironmentPolicySummary {
                mode: EnvironmentPolicyMode::RecordOnly,
                status: EnvironmentPolicyStatus::Pass,
                checked: true,
                command_results: vec![],
                warnings_count: 0,
                violations_count: 0,
                redacted_variables_count: 0,
            },
            reason: None,
        };

        let admission = AdmissionVerdictManifest {
            schema_version: 1,
            admission_run_id: "admission-1".to_string(),
            contract_path: "examples/ccl-admission-task-contract.json".to_string(),
            contract_sha256: "abc".to_string(),
            repo_path: repo.to_string_lossy().into_owned(),
            validation_manifest_path: ".ccl/runs/validation-1/validation-run-manifest.json"
                .to_string(),
            scope_manifest_path: ".ccl/runs/scope-1/scope-check-manifest.json".to_string(),
            ledger_path: "ledger/project-ledger.md".to_string(),
            status: AdmissionStatus::Pass,
            started_unix_ms: 1,
            finished_unix_ms: 2,
            github_ci_used_as_evidence: false,
            evidence: AdmissionEvidenceSummary {
                validation_status: "PASS".to_string(),
                scope_status: "PASS".to_string(),
                validation_github_ci_used_as_evidence: false,
                scope_github_ci_used_as_evidence: false,
                scope_violations_count: 0,
                validation_commands_count: 0,
                required_validation_failures_count: 0,
                missing_command_result_artifacts_count: 0,
                ledger_verification_status: Some("PASS".to_string()),
                ledger_verification_manifest_path: Some(
                    ".ccl/runs/ledger-1/ledger-verification-manifest.json".to_string(),
                ),
                ledger_exists: true,
                ledger_update_required: true,
                contract_sha256_matches_validation: true,
                contract_sha256_matches_scope: true,
            },
            violations: vec![],
            warnings: vec![],
            decision_rule: "validation PASS + scope PASS + no hard violations".to_string(),
            reason: None,
        };

        fs::write(
            validation_dir.join("validation-run-manifest.json"),
            serde_json::to_vec_pretty(&validation).unwrap(),
        )
        .unwrap();
        fs::write(
            admission_dir.join("admission-verdict.json"),
            serde_json::to_vec_pretty(&admission).unwrap(),
        )
        .unwrap();

        let loaded_validation =
            load_validation_manifest(&repo, ".ccl/runs/validation-1/validation-run-manifest.json")
                .expect("validation manifest should load");
        let loaded_admission =
            load_admission_manifest(&repo, ".ccl/runs/admission-1/admission-verdict.json")
                .expect("admission should load");

        assert_eq!(
            loaded_validation.environment_policy.status,
            EnvironmentPolicyStatus::Pass
        );
        assert_eq!(
            loaded_admission
                .evidence
                .ledger_verification_manifest_path
                .as_deref(),
            Some(".ccl/runs/ledger-1/ledger-verification-manifest.json")
        );
    }
}
