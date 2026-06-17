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
