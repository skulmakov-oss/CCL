use ccl_core::capture::{capture_command, CaptureError};
use ccl_core::evidence::{CapturePolicy, CaptureRequest, CommandSpec};
use ccl_core::preflight;
use ccl_core::task_contract::TaskContract;
use ccl_core::validation_runner::{self, ValidationRunStatus};
use ccl_core::verdict::VerdictStatus;
use clap::{Parser, Subcommand};
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
    println!();
    println!("Manifest:");
    println!("{}", manifest_path);
    println!();
    println!("GitHub CI used as evidence: NO");
}

fn validation_exit_code(status: &ValidationRunStatus) -> i32 {
    match status {
        ValidationRunStatus::Pass => 0,
        ValidationRunStatus::PassWithWarnings => 10,
        ValidationRunStatus::Fail => 20,
        ValidationRunStatus::ContractFail => 30,
    }
}
