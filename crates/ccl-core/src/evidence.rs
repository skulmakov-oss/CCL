#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceStatus {
    Captured,
    Missing,
    Failed,
}

#[derive(Debug, Clone)]
pub struct CommandEvidence {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub status: EvidenceStatus,
}

#[derive(Debug, Clone)]
pub struct ValidationEvidence {
    pub commands: Vec<CommandEvidence>,
}
