use crate::environment::EnvironmentPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredContext {
    pub dna: Option<bool>,
    pub agent_skills: Option<bool>,
    pub latest_prs: Option<u32>,
    pub project_ledger: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    AuditOnly,
    SourcePr,
    CloseoutPr,
    TestGate,
    GuardGate,
    DocsGate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContract {
    pub project: String,
    pub workstream: String,
    pub task_type: TaskType,
    pub objective: String,
    pub required_context: Option<RequiredContext>,
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub forbidden_paths: Vec<String>,
    pub required_validation: Vec<String>,
    #[serde(default)]
    pub validation: ValidationPlan,
    #[serde(default)]
    pub scope_limits: ScopeLimits,
    #[serde(default)]
    pub environment_policy: Option<EnvironmentPolicy>,
    pub github_ci_as_evidence: bool,
    pub ledger_update_required: bool,
    #[serde(default)]
    pub verdicts: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationPlan {
    #[serde(default)]
    pub commands: Vec<ValidationCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCommand {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default = "default_validation_wall_timeout_seconds")]
    pub wall_timeout_seconds: u64,
}

fn default_required() -> bool {
    true
}

fn default_validation_wall_timeout_seconds() -> u64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeLimits {
    #[serde(default = "default_max_changed_files")]
    pub max_changed_files: usize,
    #[serde(default = "default_max_diff_lines")]
    pub max_diff_lines: usize,
}

impl Default for ScopeLimits {
    fn default() -> Self {
        Self {
            max_changed_files: default_max_changed_files(),
            max_diff_lines: default_max_diff_lines(),
        }
    }
}

fn default_max_changed_files() -> usize {
    25
}

fn default_max_diff_lines() -> usize {
    1500
}

#[derive(Debug, Clone)]
pub struct ContractValidationWarning(pub String);

#[derive(Debug, Clone)]
pub struct ContractValidationError(pub String);

#[derive(Debug, Clone)]
pub struct ContractValidationReport {
    pub status: crate::verdict::VerdictStatus,
    pub errors: Vec<ContractValidationError>,
    pub warnings: Vec<ContractValidationWarning>,
}

impl TaskContract {
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    pub fn type_as_string(&self) -> String {
        serde_json::to_string(&self.task_type)
            .unwrap_or_else(|_| "\"unknown\"".to_string())
            .replace('\"', "")
    }

    pub fn validate(&self) -> ContractValidationReport {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.project.trim().is_empty() {
            errors.push(ContractValidationError(
                "Project must not be empty".to_string(),
            ));
        }
        if self.workstream.trim().is_empty() {
            errors.push(ContractValidationError(
                "Workstream must not be empty".to_string(),
            ));
        }
        if self.objective.trim().is_empty() {
            errors.push(ContractValidationError(
                "Objective must not be empty".to_string(),
            ));
        }
        if self.allowed_paths.is_empty() {
            errors.push(ContractValidationError(
                "Allowed paths must not be empty".to_string(),
            ));
        }
        if self.required_validation.is_empty() {
            errors.push(ContractValidationError(
                "Required validation must not be empty".to_string(),
            ));
        }
        if self.github_ci_as_evidence {
            errors.push(ContractValidationError(
                "github_ci_as_evidence must be false".to_string(),
            ));
        }
        if !self.ledger_update_required && self.task_type != TaskType::AuditOnly {
            warnings.push(ContractValidationWarning(
                "ledger_update_required is false but task is not audit_only".to_string(),
            ));
        }
        if self.forbidden_paths.is_empty() {
            warnings.push(ContractValidationWarning(
                "forbidden_paths is empty".to_string(),
            ));
        }
        if let Some(ctx) = &self.required_context {
            if let Some(prs) = ctx.latest_prs {
                if prs == 0 && self.task_type != TaskType::AuditOnly {
                    warnings.push(ContractValidationWarning(
                        "latest_prs is 0 but task is not audit_only".to_string(),
                    ));
                }
            }
        }

        let status = if !errors.is_empty() {
            crate::verdict::VerdictStatus::Fail
        } else if !warnings.is_empty() {
            crate::verdict::VerdictStatus::PassWithWarnings
        } else {
            crate::verdict::VerdictStatus::Pass
        };

        ContractValidationReport {
            status,
            errors,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_contract() -> TaskContract {
        serde_json::from_str(
            r#"{
            "project": "Semantic",
            "workstream": "R12 UI",
            "task_type": "source_pr",
            "objective": "Test",
            "required_context": {
                "latest_prs": 10
            },
            "allowed_paths": ["src/**"],
            "forbidden_paths": ["Cargo.toml"],
            "required_validation": ["cargo test"],
            "github_ci_as_evidence": false,
            "ledger_update_required": true
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn test_valid_contract() {
        let contract = valid_contract();
        let report = contract.validate();
        assert_eq!(report.status, crate::verdict::VerdictStatus::Pass);
    }

    #[test]
    fn test_empty_project_fails() {
        let mut contract = valid_contract();
        contract.project = "".to_string();
        let report = contract.validate();
        assert_eq!(report.status, crate::verdict::VerdictStatus::Fail);
        assert!(report.errors.iter().any(|e| e.0.contains("Project")));
    }

    #[test]
    fn test_github_ci_fails() {
        let mut contract = valid_contract();
        contract.github_ci_as_evidence = true;
        let report = contract.validate();
        assert_eq!(report.status, crate::verdict::VerdictStatus::Fail);
        assert!(report.errors.iter().any(|e| e.0.contains("github_ci")));
    }

    #[test]
    fn test_missing_allowed_paths_fails() {
        let mut contract = valid_contract();
        contract.allowed_paths.clear();
        let report = contract.validate();
        assert_eq!(report.status, crate::verdict::VerdictStatus::Fail);
        assert!(report.errors.iter().any(|e| e.0.contains("Allowed paths")));
    }

    #[test]
    fn test_ledger_update_warning() {
        let mut contract = valid_contract();
        contract.ledger_update_required = false;
        let report = contract.validate();
        assert_eq!(
            report.status,
            crate::verdict::VerdictStatus::PassWithWarnings
        );
        assert!(report
            .warnings
            .iter()
            .any(|w| w.0.contains("ledger_update_required is false")));
    }

    #[test]
    fn test_ledger_update_no_warning_if_audit_only() {
        let mut contract = valid_contract();
        contract.ledger_update_required = false;
        contract.task_type = TaskType::AuditOnly;
        let report = contract.validate();
        assert_eq!(report.status, crate::verdict::VerdictStatus::Pass);
    }

    #[test]
    fn test_environment_policy_defaults_to_none() {
        let contract = valid_contract();
        assert!(contract.environment_policy.is_none());
    }

    #[test]
    fn test_environment_policy_parses() {
        let contract: TaskContract = serde_json::from_str(
            r#"{
            "project": "CCL",
            "workstream": "Validation",
            "task_type": "guard_gate",
            "objective": "Test",
            "required_context": {
                "latest_prs": 10
            },
            "allowed_paths": ["src/**"],
            "forbidden_paths": ["Cargo.toml"],
            "required_validation": ["cargo test"],
            "environment_policy": {
                "mode": "warn",
                "allow": ["PATH"],
                "deny": ["RUSTFLAGS"],
                "allow_prefixes": ["CARGO_TERM_"],
                "deny_prefixes": ["GITHUB_"],
                "redact_patterns": ["TOKEN"],
                "unknown": "warn"
            },
            "github_ci_as_evidence": false,
            "ledger_update_required": true
        }"#,
        )
        .unwrap();
        assert!(contract.environment_policy.is_some());
        assert_eq!(
            contract.environment_policy.as_ref().unwrap().mode,
            crate::environment::EnvironmentPolicyMode::Warn
        );
    }

    #[test]
    fn test_environment_policy_invalid_mode_fails() {
        let parsed = serde_json::from_str::<TaskContract>(
            r#"{
            "project": "CCL",
            "workstream": "Validation",
            "task_type": "guard_gate",
            "objective": "Test",
            "required_context": {
                "latest_prs": 10
            },
            "allowed_paths": ["src/**"],
            "forbidden_paths": ["Cargo.toml"],
            "required_validation": ["cargo test"],
            "environment_policy": {
                "mode": "definitely_invalid"
            },
            "github_ci_as_evidence": false,
            "ledger_update_required": true
        }"#,
        );
        assert!(parsed.is_err());
    }
}
