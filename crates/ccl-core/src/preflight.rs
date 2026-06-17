use crate::verdict::{Verdict, VerdictStatus};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PreflightReport {
    pub repo_path: String,
    pub has_git: bool,
    pub has_readme: bool,
    pub has_docs: bool,
    pub has_examples: bool,
    pub has_cargo_toml: bool,
    pub verdict: Verdict,
}

pub fn run_preflight<P: AsRef<Path>>(repo_path: P) -> PreflightReport {
    let path = repo_path.as_ref();
    let has_git = path.join(".git").exists();
    let has_readme = path.join("README.md").exists();
    let has_docs = path.join("docs").exists();
    let has_examples = path.join("examples").exists();
    let has_cargo_toml = path.join("Cargo.toml").exists();

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if !has_git {
        errors.push("Missing .git directory".to_string());
    }
    if !has_readme {
        errors.push("Missing README.md".to_string());
    }

    if !has_docs {
        warnings.push("Missing docs/ directory".to_string());
    }
    if !has_examples {
        warnings.push("Missing examples/ directory".to_string());
    }
    if !has_cargo_toml {
        warnings.push("Missing Cargo.toml".to_string());
    }

    let status = if !errors.is_empty() {
        VerdictStatus::Fail
    } else if !warnings.is_empty() {
        VerdictStatus::PassWithWarnings
    } else {
        VerdictStatus::Pass
    };

    PreflightReport {
        repo_path: path.to_string_lossy().into_owned(),
        has_git,
        has_readme,
        has_docs,
        has_examples,
        has_cargo_toml,
        verdict: Verdict {
            status,
            errors,
            warnings,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_preflight() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ccl_test_preflight_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        // Initial empty state
        let report = run_preflight(&temp_dir);
        assert_eq!(report.verdict.status, VerdictStatus::Fail);
        assert!(!report.has_git);

        // Add required
        fs::create_dir(temp_dir.join(".git")).unwrap();
        fs::write(temp_dir.join("README.md"), "").unwrap();

        let report = run_preflight(&temp_dir);
        assert_eq!(report.verdict.status, VerdictStatus::PassWithWarnings); // Missing docs, examples, Cargo.toml

        // Add remaining
        fs::create_dir(temp_dir.join("docs")).unwrap();
        fs::create_dir(temp_dir.join("examples")).unwrap();
        fs::write(temp_dir.join("Cargo.toml"), "").unwrap();

        let report = run_preflight(&temp_dir);
        assert_eq!(report.verdict.status, VerdictStatus::Pass);

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
