use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictStatus {
    Pass,
    PassWithWarnings,
    Fail,
}

impl fmt::Display for VerdictStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerdictStatus::Pass => write!(f, "PASS"),
            VerdictStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            VerdictStatus::Fail => write!(f, "FAIL"),
        }
    }
}

impl VerdictStatus {
    pub fn is_pass_like(&self) -> bool {
        matches!(self, VerdictStatus::Pass | VerdictStatus::PassWithWarnings)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, VerdictStatus::Fail)
    }
}

#[derive(Debug, Clone)]
pub struct Verdict {
    pub status: VerdictStatus,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdmissionStatus {
    Pass,
    PassWithWarnings,
    Fail,
    ContractFail,
    InternalError,
}

impl fmt::Display for AdmissionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdmissionStatus::Pass => write!(f, "PASS"),
            AdmissionStatus::PassWithWarnings => write!(f, "PASS WITH WARNINGS"),
            AdmissionStatus::Fail => write!(f, "FAIL"),
            AdmissionStatus::ContractFail => write!(f, "CONTRACT_FAIL"),
            AdmissionStatus::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

impl AdmissionStatus {
    pub fn is_pass_like(&self) -> bool {
        matches!(
            self,
            AdmissionStatus::Pass | AdmissionStatus::PassWithWarnings
        )
    }

    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            AdmissionStatus::Fail | AdmissionStatus::ContractFail | AdmissionStatus::InternalError
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verdict_status_methods() {
        assert!(VerdictStatus::Pass.is_pass_like());
        assert!(!VerdictStatus::Pass.is_failure());

        assert!(VerdictStatus::PassWithWarnings.is_pass_like());
        assert!(!VerdictStatus::PassWithWarnings.is_failure());

        assert!(!VerdictStatus::Fail.is_pass_like());
        assert!(VerdictStatus::Fail.is_failure());
    }

    #[test]
    fn test_verdict_status_display() {
        assert_eq!(VerdictStatus::Pass.to_string(), "PASS");
        assert_eq!(
            VerdictStatus::PassWithWarnings.to_string(),
            "PASS WITH WARNINGS"
        );
        assert_eq!(VerdictStatus::Fail.to_string(), "FAIL");
    }
}
