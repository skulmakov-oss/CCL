use ccl_core::preflight;
use ccl_core::task_contract::TaskContract;
use ccl_core::verdict::VerdictStatus;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
}

#[derive(Subcommand)]
enum ContractCommands {
    Check { path: PathBuf },
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
        None => {}
    }
}
