use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandStatus {
    Pass,
    Fail,
}

impl fmt::Display for CommandStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandStatus::Pass => write!(f, "PASS"),
            CommandStatus::Fail => write!(f, "FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    TimeoutExceeded,
    OutputLimitExceeded,
    NonZeroExit,
    StreamDrainFailed,
    IoError,
    SpawnFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HashScope {
    SavedBytesOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePolicy {
    pub wall_timeout_seconds: u64,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
    pub max_combined_output_bytes: u64,
    pub on_output_limit: OutputLimitPolicy,
    pub capture_env: bool,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            wall_timeout_seconds: 300,
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            max_combined_output_bytes: 20 * 1024 * 1024,
            on_output_limit: OutputLimitPolicy::FailAndTerminate,
            capture_env: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputLimitPolicy {
    FailAndTerminate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequest {
    pub id: String,
    pub repo: PathBuf,
    pub command: CommandSpec,
    pub policy: CapturePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamCaptureResult {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub complete: bool,
    pub truncated: bool,
    pub hash_scope: HashScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCaptureResult {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub status: CommandStatus,
    pub failure_class: Option<FailureClass>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_limit_exceeded: bool,
    pub runtime_ms: u128,
    pub wall_timeout_seconds: u64,
    pub stdout: StreamCaptureResult,
    pub stderr: StreamCaptureResult,
    pub combined_output_bytes: u64,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
    pub max_combined_output_bytes: u64,
    pub env_path: String,
    pub env_sha256: String,
    pub result_path: String,
    pub command_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSnapshot {
    pub variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEvidenceEntry {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub status: CommandStatus,
    pub failure_class: Option<FailureClass>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_limit_exceeded: bool,
    pub runtime_ms: u128,
    pub result_path: String,
    pub command_path: String,
    pub stdout_path: String,
    pub stderr_path: String,
    pub env_path: String,
    pub env_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceManifest {
    pub run_id: String,
    pub repo_path: String,
    pub created_unix_ms: u128,
    pub aggregate_status: CommandStatus,
    pub commands: Vec<CommandEvidenceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub repo_path: String,
    pub created_unix_ms: u128,
    pub command_ids: Vec<String>,
    pub run_path: String,
    pub run_json_path: String,
    pub evidence_manifest_path: String,
}
