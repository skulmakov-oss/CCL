use crate::task_contract::TaskContract;
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct LedgerVerificationRequest {
    pub contract_path: PathBuf,
    pub repo: PathBuf,
    pub ledger_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LedgerVerificationStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
}

impl fmt::Display for LedgerVerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerVerificationStatus::Pass => write!(f, "PASS"),
            LedgerVerificationStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            LedgerVerificationStatus::Fail => write!(f, "FAIL"),
            LedgerVerificationStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
        }
    }
}

impl LedgerVerificationStatus {
    pub fn is_pass_like(&self) -> bool {
        matches!(
            self,
            LedgerVerificationStatus::Pass | LedgerVerificationStatus::PassWithWarnings
        )
    }

    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            LedgerVerificationStatus::Fail | LedgerVerificationStatus::ContractFail
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerExpectation {
    pub project: String,
    pub workstream: String,
    pub task_type: String,
    pub objective: String,
    pub ledger_update_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerMatchedEntry {
    pub heading: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerCheck {
    pub kind: String,
    pub status: String,
    pub expected: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerViolation {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerWarning {
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerVerificationManifest {
    pub schema_version: u32,
    pub ledger_run_id: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub repo_path: String,
    pub ledger_path: String,
    pub status: LedgerVerificationStatus,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
    pub github_ci_used_as_evidence: bool,
    pub expectations: LedgerExpectation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_entry: Option<LedgerMatchedEntry>,
    #[serde(default)]
    pub checks: Vec<LedgerCheck>,
    #[serde(default)]
    pub violations: Vec<LedgerViolation>,
    #[serde(default)]
    pub warnings: Vec<LedgerWarning>,
    pub decision_rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerVerificationOutcome {
    pub manifest: LedgerVerificationManifest,
    pub manifest_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LedgerVerificationError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
struct LedgerEntry {
    heading: String,
    start_line: usize,
    end_line: usize,
    text: String,
}

pub fn run_ledger_verification(
    request: LedgerVerificationRequest,
) -> Result<LedgerVerificationOutcome, LedgerVerificationError> {
    let ledger_run_id = generate_ledger_run_id();
    let repo_root = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_dir = repo_root.join(".ccl").join("runs").join(&ledger_run_id);
    fs::create_dir_all(&run_dir)?;
    let manifest_path = run_dir.join("ledger-verification-manifest.json");
    let started_unix_ms = system_time_ms(SystemTime::now());

    let contract_path_string = request.contract_path.to_string_lossy().into_owned();
    let repo_path_string = request.repo.to_string_lossy().into_owned();
    let ledger_path_string = request.ledger_path.to_string_lossy().into_owned();

    let contract_path = resolve_input_path(&repo_root, &request.contract_path);
    let contract_bytes = match fs::read(&contract_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let manifest = build_contract_fail_manifest(
                LedgerManifestBase {
                    ledger_run_id,
                    contract_path: contract_path_string,
                    contract_sha256: String::new(),
                    repo_path: repo_path_string,
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
                LedgerManifestBase {
                    ledger_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
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
            LedgerManifestBase {
                ledger_run_id,
                contract_path: contract_path_string,
                contract_sha256,
                repo_path: repo_path_string,
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
        .map(|warning| LedgerWarning {
            kind: "contract_warning".to_string(),
            reason: warning.0,
        })
        .collect::<Vec<_>>();
    let mut violations = Vec::new();
    let mut checks = Vec::new();

    let ledger_path = resolve_input_path(&repo_root, &request.ledger_path);
    let ledger_exists = ledger_path.is_file();
    if !ledger_exists {
        if contract.ledger_update_required {
            let manifest = build_contract_fail_manifest(
                LedgerManifestBase {
                    ledger_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                Some(format!("ledger_missing: {}", ledger_path.display())),
            );
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }

        warnings.push(LedgerWarning {
            kind: "ledger_missing_optional".to_string(),
            reason: format!("ledger file missing: {}", ledger_path.display()),
        });

        let manifest = finalize_manifest(LedgerManifestBuild {
            base: LedgerManifestBase {
                ledger_run_id,
                contract_path: contract_path_string,
                contract_sha256,
                repo_path: repo_path_string,
                ledger_path: ledger_path_string,
                started_unix_ms,
            },
            status: LedgerVerificationStatus::PassWithWarnings,
            contract,
            matched_entry: None,
            checks,
            violations,
            warnings,
            decision_rule: "ledger file missing but not required".to_string(),
            reason: None,
        });
        write_manifest(&manifest_path, &manifest)?;
        return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
    }

    let ledger_text = match fs::read_to_string(&ledger_path) {
        Ok(text) => text,
        Err(error) => {
            if contract.ledger_update_required {
                let manifest = build_contract_fail_manifest(
                    LedgerManifestBase {
                        ledger_run_id,
                        contract_path: contract_path_string,
                        contract_sha256,
                        repo_path: repo_path_string,
                        ledger_path: ledger_path_string,
                        started_unix_ms,
                    },
                    Some(format!("ledger_read_failed: {}", error)),
                );
                write_manifest(&manifest_path, &manifest)?;
                return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
            }

            warnings.push(LedgerWarning {
                kind: "ledger_read_failed_optional".to_string(),
                reason: format!("ledger read failed: {}", error),
            });
            let manifest = finalize_manifest(LedgerManifestBuild {
                base: LedgerManifestBase {
                    ledger_run_id,
                    contract_path: contract_path_string,
                    contract_sha256,
                    repo_path: repo_path_string,
                    ledger_path: ledger_path_string,
                    started_unix_ms,
                },
                status: LedgerVerificationStatus::PassWithWarnings,
                contract,
                matched_entry: None,
                checks,
                violations,
                warnings,
                decision_rule: "ledger read failed but not required".to_string(),
                reason: Some("ledger_read_failed_optional".to_string()),
            });
            write_manifest(&manifest_path, &manifest)?;
            return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
        }
    };

    let entries = parse_entries(&ledger_text);
    let maybe_match = select_best_entry(&entries, &contract);
    let matched_entry = maybe_match.as_ref().map(|entry| LedgerMatchedEntry {
        heading: entry.heading.clone(),
        start_line: entry.start_line,
        end_line: entry.end_line,
    });

    if maybe_match.is_none() {
        let kind = "relevant_ledger_entry_not_found".to_string();
        let reason = "no ledger entry matched the contract markers".to_string();
        if contract.ledger_update_required {
            violations.push(LedgerViolation {
                kind,
                reason: reason.clone(),
            });
        } else {
            warnings.push(LedgerWarning {
                kind,
                reason: reason.clone(),
            });
        }

        let status = if contract.ledger_update_required {
            LedgerVerificationStatus::Fail
        } else {
            LedgerVerificationStatus::PassWithWarnings
        };
        let manifest = finalize_manifest(LedgerManifestBuild {
            base: LedgerManifestBase {
                ledger_run_id,
                contract_path: contract_path_string,
                contract_sha256,
                repo_path: repo_path_string,
                ledger_path: ledger_path_string,
                started_unix_ms,
            },
            status,
            contract,
            matched_entry: None,
            checks,
            violations,
            warnings,
            decision_rule: "entry-local marker match".to_string(),
            reason: Some(reason),
        });
        write_manifest(&manifest_path, &manifest)?;
        return Ok(outcome_from_manifest(&repo_root, manifest_path, manifest));
    }

    let entry = maybe_match.unwrap();
    let entry_text_lower = entry.text.to_lowercase();
    let objective_keywords = objective_keywords(&contract.objective);
    let proof_markers = proof_markers_for_contract(&contract);

    let project_ok = contains_case_insensitive(&entry.text, &contract.project);
    push_check(
        &mut checks,
        "project_marker",
        project_ok,
        &contract.project,
        if project_ok {
            format!("found project marker {}", contract.project)
        } else {
            format!("missing project marker {}", contract.project)
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let workstream_ok = contains_case_insensitive(&entry.text, &contract.workstream);
    push_check(
        &mut checks,
        "workstream_marker",
        workstream_ok,
        &contract.workstream,
        if workstream_ok {
            format!("found workstream marker {}", contract.workstream)
        } else {
            format!("missing workstream marker {}", contract.workstream)
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let task_type_markers = task_type_required_markers(&contract);
    let task_type_ok = task_type_markers
        .iter()
        .any(|marker| contains_case_insensitive(&entry.text, marker));
    push_check(
        &mut checks,
        "task_type_marker",
        task_type_ok,
        &task_type_markers.join(" | "),
        if task_type_ok {
            format!(
                "found task type marker(s): {}",
                task_type_markers.join(", ")
            )
        } else {
            format!(
                "missing task type marker(s): {}",
                task_type_markers.join(", ")
            )
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let objective_hits = objective_keywords
        .iter()
        .filter(|keyword| contains_word(&entry_text_lower, keyword))
        .cloned()
        .collect::<Vec<_>>();
    let objective_ok = !objective_hits.is_empty();
    push_check(
        &mut checks,
        "objective_marker",
        objective_ok,
        &contract.objective,
        if objective_ok {
            format!("matched objective keywords: {}", objective_hits.join(", "))
        } else {
            "missing objective keywords".to_string()
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let github_ci_ok = contains_case_insensitive(&entry.text, "GitHub CI used as evidence: NO");
    push_check(
        &mut checks,
        "github_ci_not_evidence_marker",
        github_ci_ok,
        "GitHub CI used as evidence: NO",
        if github_ci_ok {
            "found GitHub CI used as evidence: NO".to_string()
        } else {
            "missing GitHub CI used as evidence: NO".to_string()
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let proof_ok = proof_markers
        .iter()
        .any(|marker| contains_case_insensitive(&entry.text, marker));
    push_check(
        &mut checks,
        "proof_marker",
        proof_ok,
        &proof_markers.join(" | "),
        if proof_ok {
            "found required proof marker".to_string()
        } else {
            "missing required proof marker".to_string()
        },
        &mut violations,
        contract.ledger_update_required,
    );

    let optional_markers = [
        ("pr_marker", "PR:"),
        ("branch_marker", "Branch:"),
        ("base_main_head_marker", "Base main HEAD:"),
        ("next_gate_marker", "Next Gate:"),
        ("manifest_path_marker", "manifest path:"),
    ];
    for (kind, marker) in optional_markers {
        let present = contains_case_insensitive(&entry.text, marker);
        if present {
            checks.push(LedgerCheck {
                kind: kind.to_string(),
                status: "PASS".to_string(),
                expected: marker.to_string(),
                reason: format!("found optional marker {}", marker),
            });
        } else {
            let warning = LedgerWarning {
                kind: format!("{}_missing", kind),
                reason: format!("optional marker missing: {}", marker),
            };
            warnings.push(warning.clone());
            checks.push(LedgerCheck {
                kind: kind.to_string(),
                status: "WARN".to_string(),
                expected: marker.to_string(),
                reason: format!("optional marker missing: {}", marker),
            });
        }
    }

    let status = if !violations.is_empty() {
        LedgerVerificationStatus::Fail
    } else if !warnings.is_empty() {
        LedgerVerificationStatus::PassWithWarnings
    } else {
        LedgerVerificationStatus::Pass
    };

    let reason = if violations.is_empty() {
        None
    } else {
        Some(
            violations
                .first()
                .map(|violation| violation.kind.clone())
                .unwrap_or_else(|| "ledger_verification_failure".to_string()),
        )
    };

    let manifest = finalize_manifest(LedgerManifestBuild {
        base: LedgerManifestBase {
            ledger_run_id,
            contract_path: contract_path_string,
            contract_sha256,
            repo_path: repo_path_string,
            ledger_path: ledger_path_string,
            started_unix_ms,
        },
        status,
        contract,
        matched_entry,
        checks,
        violations,
        warnings,
        decision_rule: "entry-local marker match".to_string(),
        reason,
    });

    write_manifest(&manifest_path, &manifest)?;
    Ok(outcome_from_manifest(&repo_root, manifest_path, manifest))
}

struct LedgerManifestBase {
    ledger_run_id: String,
    contract_path: String,
    contract_sha256: String,
    repo_path: String,
    ledger_path: String,
    started_unix_ms: u128,
}

struct LedgerManifestBuild {
    base: LedgerManifestBase,
    status: LedgerVerificationStatus,
    contract: TaskContract,
    matched_entry: Option<LedgerMatchedEntry>,
    checks: Vec<LedgerCheck>,
    violations: Vec<LedgerViolation>,
    warnings: Vec<LedgerWarning>,
    decision_rule: String,
    reason: Option<String>,
}

fn finalize_manifest(build: LedgerManifestBuild) -> LedgerVerificationManifest {
    let LedgerManifestBuild {
        base,
        status,
        contract,
        matched_entry,
        checks,
        violations,
        warnings,
        decision_rule,
        reason,
    } = build;
    let task_type = contract.type_as_string();
    LedgerVerificationManifest {
        schema_version: 1,
        ledger_run_id: base.ledger_run_id,
        contract_path: base.contract_path,
        contract_sha256: base.contract_sha256,
        repo_path: base.repo_path,
        ledger_path: base.ledger_path,
        status,
        started_unix_ms: base.started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        expectations: LedgerExpectation {
            project: contract.project,
            workstream: contract.workstream,
            task_type,
            objective: contract.objective,
            ledger_update_required: contract.ledger_update_required,
        },
        matched_entry,
        checks,
        violations,
        warnings,
        decision_rule,
        reason,
    }
}

fn build_contract_fail_manifest(
    base: LedgerManifestBase,
    reason: Option<String>,
) -> LedgerVerificationManifest {
    LedgerVerificationManifest {
        schema_version: 1,
        ledger_run_id: base.ledger_run_id,
        contract_path: base.contract_path,
        contract_sha256: base.contract_sha256,
        repo_path: base.repo_path,
        ledger_path: base.ledger_path,
        status: LedgerVerificationStatus::ContractFail,
        started_unix_ms: base.started_unix_ms,
        finished_unix_ms: system_time_ms(SystemTime::now()),
        github_ci_used_as_evidence: false,
        expectations: LedgerExpectation {
            project: String::new(),
            workstream: String::new(),
            task_type: String::new(),
            objective: String::new(),
            ledger_update_required: true,
        },
        matched_entry: None,
        checks: vec![],
        violations: reason
            .clone()
            .into_iter()
            .map(|reason| LedgerViolation {
                kind: "contract_fail".to_string(),
                reason,
            })
            .collect(),
        warnings: vec![],
        decision_rule: "contract validation".to_string(),
        reason,
    }
}

fn push_check(
    checks: &mut Vec<LedgerCheck>,
    kind: &str,
    ok: bool,
    expected: &str,
    reason: String,
    violations: &mut Vec<LedgerViolation>,
    required: bool,
) {
    checks.push(LedgerCheck {
        kind: kind.to_string(),
        status: if ok {
            "PASS".to_string()
        } else {
            "FAIL".to_string()
        },
        expected: expected.to_string(),
        reason: reason.clone(),
    });
    if !ok && required {
        violations.push(LedgerViolation {
            kind: kind.to_string(),
            reason,
        });
    }
}

fn outcome_from_manifest(
    repo_root: &Path,
    manifest_path: PathBuf,
    manifest: LedgerVerificationManifest,
) -> LedgerVerificationOutcome {
    LedgerVerificationOutcome {
        manifest_path: repo_relative_string(repo_root, manifest_path),
        manifest,
    }
}

fn write_manifest(
    path: &Path,
    manifest: &LedgerVerificationManifest,
) -> Result<(), LedgerVerificationError> {
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn parse_entries(text: &str) -> Vec<LedgerEntry> {
    let mut entries = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_lines = Vec::new();
    let mut current_start = 0usize;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.starts_with("## ") {
            if let Some(heading) = current_heading.take() {
                entries.push(LedgerEntry {
                    heading,
                    start_line: current_start,
                    end_line: line_number.saturating_sub(1),
                    text: current_lines.join("\n"),
                });
                current_lines.clear();
            }
            current_heading = Some(line.trim().to_string());
            current_start = line_number;
            current_lines.push(line.to_string());
        } else if current_heading.is_some() {
            current_lines.push(line.to_string());
        }
    }

    if let Some(heading) = current_heading {
        entries.push(LedgerEntry {
            heading,
            start_line: current_start,
            end_line: text.lines().count(),
            text: current_lines.join("\n"),
        });
    }

    entries
}

fn select_best_entry<'a>(
    entries: &'a [LedgerEntry],
    contract: &TaskContract,
) -> Option<&'a LedgerEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            if !hard_entry_matches_contract(entry, contract) {
                return None;
            }
            let score = score_entry(entry, contract);
            if score >= minimum_score(contract) {
                Some((score, entry))
            } else {
                None
            }
        })
        .max_by(|(score_a, entry_a), (score_b, entry_b)| {
            score_a
                .cmp(score_b)
                .then_with(|| entry_a.start_line.cmp(&entry_b.start_line))
        })
        .map(|(_, entry)| entry)
}

fn minimum_score(contract: &TaskContract) -> i32 {
    let base = 8;
    if contract.ledger_update_required {
        base
    } else {
        base - 2
    }
}

fn hard_entry_matches_contract(entry: &LedgerEntry, contract: &TaskContract) -> bool {
    let task_type_markers = task_type_required_markers(contract);
    let objective_phrases = objective_required_phrases(contract);
    let proof_markers = proof_markers_for_contract(contract);

    contains_case_insensitive(&entry.text, &contract.project)
        && contains_case_insensitive(&entry.text, &contract.workstream)
        && task_type_markers
            .iter()
            .any(|marker| contains_case_insensitive(&entry.text, marker))
        && contains_case_insensitive(&entry.text, "Status:")
        && contains_case_insensitive(&entry.text, "GitHub CI used as evidence: NO")
        && objective_phrases
            .iter()
            .any(|phrase| contains_case_insensitive(&entry.text, phrase))
        && proof_markers
            .iter()
            .any(|marker| contains_case_insensitive(&entry.text, marker))
}

fn score_entry(entry: &LedgerEntry, contract: &TaskContract) -> i32 {
    let mut score = 0;
    if contains_case_insensitive(&entry.text, &contract.project) {
        score += 5;
    }
    if contains_case_insensitive(&entry.text, &contract.workstream) {
        score += 5;
    }
    if contains_case_insensitive(&entry.text, "status:") {
        score += 2;
    }
    if contains_case_insensitive(&entry.text, "task type:") {
        score += 1;
    }
    for marker in task_type_required_markers(contract) {
        if contains_case_insensitive(&entry.text, &marker) {
            score += 3;
        }
    }
    if contains_case_insensitive(&entry.text, "github ci used as evidence: no") {
        score += 3;
    }
    if contains_case_insensitive(&entry.text, "branch:") {
        score += 1;
    }
    if contains_case_insensitive(&entry.text, "pr:") {
        score += 1;
    }
    if contains_case_insensitive(&entry.text, "base main head:") {
        score += 1;
    }
    for keyword in objective_keywords(&contract.objective) {
        if contains_word(&entry.text.to_lowercase(), &keyword) {
            score += 1;
        }
    }
    for phrase in objective_required_phrases(contract) {
        if contains_case_insensitive(&entry.text, &phrase) {
            score += 3;
        }
    }
    for marker in proof_markers_for_contract(contract) {
        if contains_case_insensitive(&entry.text, marker) {
            score += 2;
        }
    }
    score
}

fn task_type_required_markers(contract: &TaskContract) -> Vec<String> {
    let exact = contract.type_as_string();
    match contract.task_type {
        crate::task_contract::TaskType::GuardGate => vec![
            format!("Task type: {}", exact),
            "Task type: admission verdict".to_string(),
            "Task type: gate orchestration".to_string(),
        ],
        _ => vec![format!("Task type: {}", exact)],
    }
}

fn objective_required_phrases(contract: &TaskContract) -> Vec<String> {
    let objective = contract.objective.to_lowercase();
    let mut phrases = Vec::new();
    let preferred_phrases = [
        "admission verdict",
        "validation and scope evidence",
        "existing validation",
        "scope evidence",
        "ledger semantic verification",
        "gate orchestration",
        "scope diff policy",
        "command evidence capture",
    ];

    for phrase in preferred_phrases {
        if objective.contains(phrase) {
            phrases.push(phrase.to_string());
        }
    }

    if phrases.is_empty() {
        let keywords = objective_keywords(&objective);
        if keywords.len() >= 2 {
            phrases.push(format!("{} {}", keywords[0], keywords[1]));
        } else if let Some(keyword) = keywords.first() {
            phrases.push(keyword.clone());
        }
    }

    phrases
}

fn objective_keywords(objective: &str) -> Vec<String> {
    let stop_words = [
        "the", "and", "with", "from", "that", "this", "into", "only", "when", "will", "must",
        "into", "existing", "compute", "seed", "seed.", "task", "work", "for", "are", "was", "be",
        "not", "can", "may", "should", "are", "either", "after", "before",
    ];
    let mut keywords = Vec::new();
    for token in objective
        .split(|c: char| !c.is_alphanumeric())
        .map(|token| token.trim().to_lowercase())
        .filter(|token| token.len() >= 4)
    {
        if stop_words.contains(&token.as_str()) {
            continue;
        }
        if !keywords.contains(&token) {
            keywords.push(token);
        }
    }
    keywords
}

fn proof_markers_for_contract(contract: &TaskContract) -> Vec<&'static str> {
    let mut markers = vec![
        "Gate Proof",
        "Admission Proof",
        "Scope Check Proof",
        "Validation runner proof",
        "Capture Proof",
    ];
    let objective = contract.objective.to_lowercase();
    if objective.contains("ledger semantic verification") || objective.contains("ledger verify") {
        markers.push("Ledger Verification Proof");
    }
    markers
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack
        .split(|c: char| !c.is_alphanumeric())
        .any(|token| token == needle)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
}

fn system_time_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
}

fn generate_ledger_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("ledger-{}-{}", now, std::process::id())
}

fn resolve_input_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
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

fn normalize_path_string(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_repo(name: &str) -> PathBuf {
        let repo = std::env::temp_dir().join(format!(
            "ccl-ledger-{}-{}-{}",
            name,
            std::process::id(),
            system_time_ms(SystemTime::now())
        ));
        fs::create_dir_all(&repo).unwrap();
        repo
    }

    fn write_contract(repo: &Path) -> PathBuf {
        let contract_path = repo.join("examples/ccl-admission-task-contract.json");
        if let Some(parent) = contract_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(
            &contract_path,
            include_str!("../../../examples/ccl-admission-task-contract.json"),
        )
        .unwrap();
        contract_path
    }

    fn write_ledger(repo: &Path, text: &str) -> PathBuf {
        let ledger_path = repo.join("ledger/project-ledger.md");
        if let Some(parent) = ledger_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&ledger_path, text).unwrap();
        ledger_path
    }

    fn run(repo: &Path) -> LedgerVerificationOutcome {
        run_ledger_verification(LedgerVerificationRequest {
            contract_path: PathBuf::from("examples/ccl-admission-task-contract.json"),
            repo: repo.to_path_buf(),
            ledger_path: PathBuf::from("ledger/project-ledger.md"),
        })
        .unwrap()
    }

    fn scope_diff_ledger_text() -> String {
        r#"# CCL Project Ledger

## 2026-06-21 — Scope/Diff Policy Check Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: CCL Phase 1
- Task type: scope policy
- Branch: feat/scope-diff-policy-check-seed
- PR: #8
- Base main HEAD: fc569f1127cd2352771d5c88a3ef885973fdb5a2

### Validation

- GitHub CI used as evidence: NO

### Scope Check Proof

- contract path: examples/ccl-admission-task-contract.json
- command: cargo run -p ccl-cli -- scope check --contract examples/ccl-admission-task-contract.json --repo .
- status: PASS
- scope manifest path: .ccl/runs/scope-1/scope-check-manifest.json

### Boundary Conclusion

- scope checker added: YES
- GitHub CI used as evidence: NO
"#
        .to_string()
    }

    fn admission_ledger_text() -> String {
        r#"# CCL Project Ledger

## 2026-06-21 — Scope/Diff Policy Check Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: CCL Phase 1
- Task type: scope policy
- Branch: feat/scope-diff-policy-check-seed
- PR: #8
- Base main HEAD: fc569f1127cd2352771d5c88a3ef885973fdb5a2

### Validation

- GitHub CI used as evidence: NO

### Scope Check Proof

- contract path: examples/ccl-admission-task-contract.json
- command: cargo run -p ccl-cli -- scope check --contract examples/ccl-admission-task-contract.json --repo .
- status: PASS
- scope manifest path: .ccl/runs/scope-1/scope-check-manifest.json

### Boundary Conclusion

- scope checker added: YES
- GitHub CI used as evidence: NO

## 2026-06-21 — Admission Verdict From Evidence Seed

Status: PASS WITH WARNINGS

### Scope

- Workstream: Admission
- Task type: guard_gate
- Branch: feat/admission-verdict-from-evidence-seed
- PR: #9
- Base main HEAD: fc569f1127cd2352771d5c88a3ef885973fdb5a2

### Objective

- Objective: Compute an admission verdict from existing validation and scope evidence.

### Validation

- GitHub CI used as evidence: NO

### Next Gate

- recommended next gate: Gate Orchestration Seed
- reason: admission verdicts are now derived mechanically from evidence, so the next layer is a single orchestrator over the existing deterministic steps

### Admission Proof

- contract path: examples/ccl-admission-task-contract.json
- command: cargo run -p ccl-cli -- admission verdict --contract examples/ccl-admission-task-contract.json --repo . --validation-manifest .ccl/runs/validation-1/validation-run-manifest.json --scope-manifest .ccl/runs/scope-1/scope-check-manifest.json
- status: PASS
- admission verdict path: .ccl/runs/admission-1/admission-verdict.json
- ledger verification manifest path: .ccl/runs/ledger-1/ledger-verification-manifest.json

### Boundary Conclusion

- admission verdict command added: YES
- validation manifest consumed: YES
- scope manifest consumed: YES
- admission verdict invoked: YES
- GitHub CI used as evidence: NO
"#
        .to_string()
    }

    #[test]
    fn scope_diff_entry_does_not_match_admission_contract() {
        let repo = temp_repo("scope-fail");
        write_contract(&repo);
        write_ledger(&repo, &scope_diff_ledger_text());

        let outcome = run(&repo);

        assert_eq!(outcome.manifest.status, LedgerVerificationStatus::Fail);
        assert!(outcome.manifest.matched_entry.is_none());
        assert!(outcome
            .manifest
            .violations
            .iter()
            .any(|violation| violation.kind == "relevant_ledger_entry_not_found"));
    }

    #[test]
    fn admission_entry_matches_admission_contract() {
        let repo = temp_repo("admission-pass");
        write_contract(&repo);
        write_ledger(&repo, &admission_ledger_text());

        let outcome = run(&repo);

        assert_eq!(outcome.manifest.status, LedgerVerificationStatus::Pass);
        assert_eq!(
            outcome
                .manifest
                .matched_entry
                .as_ref()
                .map(|entry| entry.heading.as_str()),
            Some("## 2026-06-21 — Admission Verdict From Evidence Seed")
        );
        assert!(outcome
            .manifest
            .checks
            .iter()
            .any(|check| check.kind == "objective_marker" && check.status == "PASS"));
        assert!(outcome
            .manifest
            .checks
            .iter()
            .any(|check| check.kind == "task_type_marker" && check.status == "PASS"));
    }
}
