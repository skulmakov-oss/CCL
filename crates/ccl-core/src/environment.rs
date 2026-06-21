use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPolicyMode {
    RecordOnly,
    Warn,
    Enforce,
    Strict,
}

impl fmt::Display for EnvironmentPolicyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvironmentPolicyMode::RecordOnly => write!(f, "record_only"),
            EnvironmentPolicyMode::Warn => write!(f, "warn"),
            EnvironmentPolicyMode::Enforce => write!(f, "enforce"),
            EnvironmentPolicyMode::Strict => write!(f, "strict"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentUnknownPolicy {
    #[default]
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPolicyStatus {
    Pass,
    Warn,
    Fail,
    ContractFail,
}

impl fmt::Display for EnvironmentPolicyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvironmentPolicyStatus::Pass => write!(f, "PASS"),
            EnvironmentPolicyStatus::Warn => write!(f, "WARN"),
            EnvironmentPolicyStatus::Fail => write!(f, "FAIL"),
            EnvironmentPolicyStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPolicyDecision {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentVariableClass {
    RequiredRuntime,
    ToolchainPath,
    ToolchainBehavior,
    TestBehavior,
    CiMetadata,
    DisplayOnly,
    CredentialOrSecret,
    NetworkProxy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentMatchRule {
    ExactDeny,
    DenyPrefix,
    ExactAllow,
    AllowPrefix,
    BaselineAllowlist,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPolicy {
    #[serde(default = "default_environment_policy_mode")]
    pub mode: EnvironmentPolicyMode,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub allow_prefixes: Vec<String>,
    #[serde(default)]
    pub deny_prefixes: Vec<String>,
    #[serde(default)]
    pub redact_patterns: Vec<String>,
    #[serde(default)]
    pub unknown: EnvironmentUnknownPolicy,
}

impl Default for EnvironmentPolicy {
    fn default() -> Self {
        Self {
            mode: default_environment_policy_mode(),
            allow: vec![],
            deny: vec![],
            allow_prefixes: vec![],
            deny_prefixes: vec![],
            redact_patterns: vec![],
            unknown: EnvironmentUnknownPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedEnvironmentVariable {
    pub name: String,
    pub class: EnvironmentVariableClass,
    pub match_rule: EnvironmentMatchRule,
    pub decision: EnvironmentPolicyDecision,
    pub redacted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPolicyViolation {
    pub variable: String,
    pub class: EnvironmentVariableClass,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPolicyWarning {
    pub variable: String,
    pub class: EnvironmentVariableClass,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPolicyResult {
    pub mode: EnvironmentPolicyMode,
    pub status: EnvironmentPolicyStatus,
    pub checked: bool,
    #[serde(default)]
    pub variables: Vec<ClassifiedEnvironmentVariable>,
    #[serde(default)]
    pub warnings: Vec<EnvironmentPolicyWarning>,
    #[serde(default)]
    pub violations: Vec<EnvironmentPolicyViolation>,
    #[serde(default)]
    pub redacted_variables: Vec<String>,
    pub allowed_variables_count: usize,
    pub denied_variables_count: usize,
    pub unknown_variables_count: usize,
}

impl Default for EnvironmentPolicyResult {
    fn default() -> Self {
        Self {
            mode: EnvironmentPolicyMode::RecordOnly,
            status: EnvironmentPolicyStatus::Pass,
            checked: false,
            variables: vec![],
            warnings: vec![],
            violations: vec![],
            redacted_variables: vec![],
            allowed_variables_count: 0,
            denied_variables_count: 0,
            unknown_variables_count: 0,
        }
    }
}

impl EnvironmentPolicy {
    pub fn record_only() -> Self {
        Self::default()
    }

    pub fn evaluate(&self, snapshot: &BTreeMap<String, String>) -> EnvironmentPolicyResult {
        let mut variables = Vec::with_capacity(snapshot.len());
        let mut warnings = Vec::new();
        let mut violations = Vec::new();
        let mut redacted_variables = Vec::new();
        let mut allowed_variables_count = 0usize;
        let mut denied_variables_count = 0usize;
        let mut unknown_variables_count = 0usize;
        let mut saw_warning = false;
        let mut saw_failure = false;

        for (name, value) in snapshot {
            let normalized_name = normalize_key(name);
            let class = classify_variable(&normalized_name);
            let redacted = should_redact(&normalized_name, &self.redact_patterns);
            if redacted {
                redacted_variables.push(name.clone());
            }

            let match_rule = self.match_rule(&normalized_name);
            let decision = self.decision_for(&match_rule, &normalized_name);
            let reason = reason_for_match(&match_rule);

            match match_rule {
                EnvironmentMatchRule::ExactDeny | EnvironmentMatchRule::DenyPrefix => {
                    denied_variables_count = denied_variables_count.saturating_add(1)
                }
                EnvironmentMatchRule::ExactAllow
                | EnvironmentMatchRule::AllowPrefix
                | EnvironmentMatchRule::BaselineAllowlist => {
                    allowed_variables_count = allowed_variables_count.saturating_add(1)
                }
                EnvironmentMatchRule::Unknown => {
                    unknown_variables_count = unknown_variables_count.saturating_add(1)
                }
            }

            if matches!(decision, EnvironmentPolicyDecision::Warn) {
                saw_warning = true;
                warnings.push(EnvironmentPolicyWarning {
                    variable: name.clone(),
                    class: class.clone(),
                    reason: reason.clone(),
                });
            } else if matches!(decision, EnvironmentPolicyDecision::Fail) {
                saw_failure = true;
                violations.push(EnvironmentPolicyViolation {
                    variable: name.clone(),
                    class: class.clone(),
                    reason: reason.clone(),
                });
            }

            variables.push(ClassifiedEnvironmentVariable {
                name: name.clone(),
                class,
                match_rule,
                decision,
                redacted,
                reason,
            });

            let _ = value;
        }

        let status = match self.mode {
            EnvironmentPolicyMode::RecordOnly => EnvironmentPolicyStatus::Pass,
            EnvironmentPolicyMode::Warn => {
                if saw_failure {
                    EnvironmentPolicyStatus::Fail
                } else if saw_warning || !violations.is_empty() {
                    EnvironmentPolicyStatus::Warn
                } else {
                    EnvironmentPolicyStatus::Pass
                }
            }
            EnvironmentPolicyMode::Enforce | EnvironmentPolicyMode::Strict => {
                if saw_failure {
                    EnvironmentPolicyStatus::Fail
                } else if saw_warning {
                    EnvironmentPolicyStatus::Warn
                } else {
                    EnvironmentPolicyStatus::Pass
                }
            }
        };

        EnvironmentPolicyResult {
            mode: self.mode.clone(),
            status,
            checked: true,
            variables,
            warnings,
            violations,
            redacted_variables,
            allowed_variables_count,
            denied_variables_count,
            unknown_variables_count,
        }
    }

    fn match_rule(&self, name: &str) -> EnvironmentMatchRule {
        if matches_exact(name, &self.deny) || matches_prefix(name, &self.deny_prefixes) {
            if matches_exact(name, &self.deny) {
                EnvironmentMatchRule::ExactDeny
            } else {
                EnvironmentMatchRule::DenyPrefix
            }
        } else if matches_exact(name, &self.allow) {
            EnvironmentMatchRule::ExactAllow
        } else if matches_prefix(name, &self.allow_prefixes) {
            EnvironmentMatchRule::AllowPrefix
        } else if baseline_allowlist().contains(&name) {
            EnvironmentMatchRule::BaselineAllowlist
        } else {
            EnvironmentMatchRule::Unknown
        }
    }

    fn decision_for(&self, rule: &EnvironmentMatchRule, name: &str) -> EnvironmentPolicyDecision {
        let decision = match self.mode {
            EnvironmentPolicyMode::RecordOnly => EnvironmentPolicyDecision::Pass,
            EnvironmentPolicyMode::Warn => match rule {
                EnvironmentMatchRule::ExactDeny | EnvironmentMatchRule::DenyPrefix => {
                    EnvironmentPolicyDecision::Warn
                }
                EnvironmentMatchRule::Unknown => EnvironmentPolicyDecision::Warn,
                _ => EnvironmentPolicyDecision::Pass,
            },
            EnvironmentPolicyMode::Enforce => match rule {
                EnvironmentMatchRule::ExactDeny | EnvironmentMatchRule::DenyPrefix => {
                    EnvironmentPolicyDecision::Fail
                }
                EnvironmentMatchRule::Unknown => match self.unknown {
                    EnvironmentUnknownPolicy::Warn => EnvironmentPolicyDecision::Warn,
                    EnvironmentUnknownPolicy::Fail => EnvironmentPolicyDecision::Fail,
                },
                _ => EnvironmentPolicyDecision::Pass,
            },
            EnvironmentPolicyMode::Strict => match rule {
                EnvironmentMatchRule::ExactDeny | EnvironmentMatchRule::DenyPrefix => {
                    EnvironmentPolicyDecision::Fail
                }
                EnvironmentMatchRule::Unknown => match self.unknown {
                    EnvironmentUnknownPolicy::Warn => EnvironmentPolicyDecision::Warn,
                    EnvironmentUnknownPolicy::Fail => EnvironmentPolicyDecision::Fail,
                },
                _ => EnvironmentPolicyDecision::Pass,
            },
        };
        let _ = name;
        decision
    }
}

fn default_environment_policy_mode() -> EnvironmentPolicyMode {
    EnvironmentPolicyMode::RecordOnly
}

fn normalize_key(name: &str) -> String {
    if cfg!(windows) {
        name.to_uppercase()
    } else {
        name.to_string()
    }
}

fn normalize_pattern(pattern: &str) -> String {
    normalize_key(pattern.trim())
}

fn matches_exact(name: &str, patterns: &[String]) -> bool {
    let normalized = normalize_key(name);
    patterns
        .iter()
        .any(|pattern| normalized == normalize_pattern(pattern))
}

fn matches_prefix(name: &str, prefixes: &[String]) -> bool {
    let normalized = normalize_key(name);
    prefixes
        .iter()
        .any(|prefix| normalized.starts_with(&normalize_pattern(prefix)))
}

fn should_redact(name: &str, patterns: &[String]) -> bool {
    let normalized = normalize_key(name);
    patterns.iter().any(|pattern| {
        let needle = normalize_pattern(pattern);
        !needle.is_empty() && normalized.contains(&needle)
    })
}

fn classify_variable(name: &str) -> EnvironmentVariableClass {
    match name {
        "PATH" | "SYSTEMROOT" | "WINDIR" | "COMSPEC" | "PATHEXT" | "HOME" | "USERPROFILE"
        | "TMP" | "TEMP" | "TMPDIR" | "SHELL" | "USER" | "LOGNAME" => {
            EnvironmentVariableClass::RequiredRuntime
        }
        "CARGO_HOME" | "RUSTUP_HOME" | "CARGO_TARGET_DIR" => {
            EnvironmentVariableClass::ToolchainPath
        }
        "RUSTFLAGS"
        | "CARGO_ENCODED_RUSTFLAGS"
        | "CARGO_TERM_COLOR"
        | "CARGO_TERM_PROGRESS_WHEN"
        | "CARGO_TERM_VERBOSE" => EnvironmentVariableClass::ToolchainBehavior,
        "RUST_TEST_THREADS" => EnvironmentVariableClass::TestBehavior,
        "CI" | "GITHUB_ACTIONS" | "ACTIONS_ID_TOKEN_REQUEST_URL" | "ACTIONS_RUNTIME_TOKEN" => {
            EnvironmentVariableClass::CiMetadata
        }
        "NO_COLOR" | "TERM" | "CLICOLOR" => EnvironmentVariableClass::DisplayOnly,
        "GITHUB_TOKEN" | "GH_TOKEN" | "TOKEN" | "SECRET" | "PASSWORD" | "KEY" => {
            EnvironmentVariableClass::CredentialOrSecret
        }
        "HTTP_PROXY" | "HTTPS_PROXY" | "ALL_PROXY" | "NO_PROXY" => {
            EnvironmentVariableClass::NetworkProxy
        }
        _ => EnvironmentVariableClass::Unknown,
    }
}

fn baseline_allowlist() -> &'static [&'static str] {
    if cfg!(windows) {
        &[
            "PATH",
            "SYSTEMROOT",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "HOME",
            "USERPROFILE",
            "TMP",
            "TEMP",
            "NO_COLOR",
            "TERM",
            "CLICOLOR",
        ]
    } else {
        &[
            "PATH", "HOME", "TMPDIR", "TMP", "TEMP", "SHELL", "USER", "LOGNAME", "NO_COLOR",
            "TERM", "CLICOLOR",
        ]
    }
}

fn reason_for_match(rule: &EnvironmentMatchRule) -> String {
    match rule {
        EnvironmentMatchRule::ExactDeny => "denied_by_exact_match".to_string(),
        EnvironmentMatchRule::DenyPrefix => "denied_by_prefix".to_string(),
        EnvironmentMatchRule::ExactAllow => "allowed_by_exact_match".to_string(),
        EnvironmentMatchRule::AllowPrefix => "allowed_by_prefix".to_string(),
        EnvironmentMatchRule::BaselineAllowlist => "allowed_by_baseline".to_string(),
        EnvironmentMatchRule::Unknown => "unknown_by_policy".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn env(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn record_only_never_fails() {
        let policy = EnvironmentPolicy::default();
        let result = policy.evaluate(&env(&[("RUSTFLAGS", "-D warnings")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Pass);
        assert_eq!(result.variables.len(), 1);
    }

    #[test]
    fn deny_exact_wins_over_allow_exact() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Warn,
            allow: vec!["RUSTFLAGS".to_string()],
            deny: vec!["RUSTFLAGS".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("RUSTFLAGS", "-D warnings")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Warn);
        assert_eq!(
            result.variables[0].match_rule,
            EnvironmentMatchRule::ExactDeny
        );
    }

    #[test]
    fn deny_prefix_wins_over_allow_prefix() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Enforce,
            allow_prefixes: vec!["CARGO_".to_string()],
            deny_prefixes: vec!["CARGO_TERM_".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("CARGO_TERM_COLOR", "always")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Fail);
        assert_eq!(
            result.variables[0].match_rule,
            EnvironmentMatchRule::DenyPrefix
        );
    }

    #[test]
    fn deny_prefix_wins_over_allow_exact() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Enforce,
            allow: vec!["CARGO_TERM_COLOR".to_string()],
            deny_prefixes: vec!["CARGO_TERM_".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("CARGO_TERM_COLOR", "always")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Fail);
        assert_eq!(
            result.variables[0].match_rule,
            EnvironmentMatchRule::DenyPrefix
        );
    }

    #[test]
    fn secret_like_variable_is_redacted() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Warn,
            redact_patterns: vec!["TOKEN".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("GITHUB_TOKEN", "secret")]));
        assert!(result.variables[0].redacted);
        assert_eq!(result.redacted_variables, vec!["GITHUB_TOKEN".to_string()]);
    }

    #[test]
    fn warn_mode_produces_warnings() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Warn,
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("CARGO_HOME", "x")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Warn);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn enforce_mode_fails_for_denied_variable() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Enforce,
            deny: vec!["RUSTFLAGS".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("RUSTFLAGS", "-D warnings")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Fail);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn strict_mode_handles_unknown_variable_deterministically() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Strict,
            unknown: EnvironmentUnknownPolicy::Fail,
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("CCL_UNKNOWN", "1")]));
        assert_eq!(result.status, EnvironmentPolicyStatus::Fail);
        assert_eq!(
            result.variables[0].match_rule,
            EnvironmentMatchRule::Unknown
        );
    }

    #[test]
    fn windows_style_case_insensitive_matching_is_covered() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Warn,
            allow: vec!["path".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("Path", "C:\\Tools")]));
        if cfg!(windows) {
            assert_eq!(
                result.variables[0].match_rule,
                EnvironmentMatchRule::ExactAllow
            );
        } else {
            assert_eq!(
                result.variables[0].match_rule,
                EnvironmentMatchRule::Unknown
            );
        }
    }

    #[test]
    fn unix_style_case_sensitive_matching_is_covered() {
        let policy = EnvironmentPolicy {
            mode: EnvironmentPolicyMode::Warn,
            allow: vec!["path".to_string()],
            ..EnvironmentPolicy::default()
        };
        let result = policy.evaluate(&env(&[("PATH", "/usr/bin")]));
        if cfg!(windows) {
            assert_eq!(
                result.variables[0].match_rule,
                EnvironmentMatchRule::ExactAllow
            );
        } else {
            assert_eq!(
                result.variables[0].match_rule,
                EnvironmentMatchRule::Unknown
            );
        }
    }
}
