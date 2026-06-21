use crate::environment::{EnvironmentPolicy, EnvironmentPolicyResult, EnvironmentPolicyStatus};
use crate::evidence::{
    CaptureRequest, CommandCaptureResult, CommandEvidenceEntry, CommandStatus, EnvSnapshot,
    EvidenceManifest, FailureClass, HashScope, OutputLimitPolicy, RunRecord, StreamCaptureResult,
};
use hex::encode as hex_encode;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
}

#[derive(Debug, Clone)]
pub struct CaptureOutcome {
    pub run: RunRecord,
    pub manifest: EvidenceManifest,
    pub command_result: CommandCaptureResult,
}

#[derive(Debug)]
struct ReaderCompletion {
    result: StreamCaptureResult,
}

#[derive(Debug, Clone)]
struct StreamRuntimeState {
    bytes: u64,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct SharedCaptureState {
    stdout: StreamRuntimeState,
    stderr: StreamRuntimeState,
    combined_output_bytes: u64,
    output_limit_exceeded: bool,
}

impl SharedCaptureState {
    fn new() -> Self {
        Self {
            stdout: StreamRuntimeState {
                bytes: 0,
                truncated: false,
            },
            stderr: StreamRuntimeState {
                bytes: 0,
                truncated: false,
            },
            combined_output_bytes: 0,
            output_limit_exceeded: false,
        }
    }
}

pub fn capture_command(request: CaptureRequest) -> Result<CaptureOutcome, CaptureError> {
    if request.id.trim().is_empty() {
        return Err(CaptureError::InvalidCommand(
            "capture id must not be empty".to_string(),
        ));
    }
    if request.command.program.trim().is_empty() {
        return Err(CaptureError::InvalidCommand(
            "program must not be empty".to_string(),
        ));
    }

    let repo = request
        .repo
        .canonicalize()
        .unwrap_or_else(|_| request.repo.clone());
    let run_id = generate_run_id();
    let created_unix_ms = system_time_ms(SystemTime::now());
    let run_dir = repo.join(".ccl").join("runs").join(&run_id);
    let command_slug = sanitize_component(&request.id);
    let command_dir = run_dir
        .join("commands")
        .join(format!("001-{}", command_slug));

    fs::create_dir_all(&command_dir)?;

    let command_path = command_dir.join("command.json");
    let env_path = command_dir.join("env.json");
    let stdout_path = command_dir.join("stdout.txt");
    let stderr_path = command_dir.join("stderr.txt");
    let result_path = command_dir.join("result.json");
    let run_json_path = run_dir.join("run.json");
    let manifest_path = run_dir.join("evidence-manifest.json");
    let artifact_paths = ArtifactPaths {
        run_dir: run_dir.clone(),
        command_path: command_path.clone(),
        env_path: env_path.clone(),
        stdout_path: stdout_path.clone(),
        stderr_path: stderr_path.clone(),
        result_path: result_path.clone(),
        run_json_path: run_json_path.clone(),
        manifest_path: manifest_path.clone(),
    };

    let env_snapshot = if request.policy.capture_env {
        EnvSnapshot {
            variables: capture_environment(),
        }
    } else {
        EnvSnapshot {
            variables: BTreeMap::new(),
        }
    };
    let env_json = serde_json::to_vec_pretty(&env_snapshot)?;
    fs::write(&env_path, &env_json)?;
    let env_sha256 = sha256_hex(&env_json);

    let environment_policy = request
        .environment_policy
        .clone()
        .unwrap_or_else(EnvironmentPolicy::record_only);
    let environment_policy_result = environment_policy.evaluate(&env_snapshot.variables);
    let environment_policy_config = environment_policy.clone();

    let command_json = CommandArtifact {
        id: request.id.clone(),
        repo_path: repo.to_string_lossy().into_owned(),
        cwd: repo.to_string_lossy().into_owned(),
        program: request.command.program.clone(),
        args: request.command.args.clone(),
        wall_timeout_seconds: request.policy.wall_timeout_seconds,
        max_stdout_bytes: request.policy.max_stdout_bytes,
        max_stderr_bytes: request.policy.max_stderr_bytes,
        max_combined_output_bytes: request.policy.max_combined_output_bytes,
        on_output_limit: request.policy.on_output_limit.clone(),
        capture_env: request.policy.capture_env,
        environment_policy: environment_policy_config.clone(),
        created_unix_ms,
    };
    write_json_file(&command_path, &command_json)?;

    if matches!(
        environment_policy_result.status,
        EnvironmentPolicyStatus::Fail
    ) && matches!(
        environment_policy.mode,
        crate::environment::EnvironmentPolicyMode::Enforce
            | crate::environment::EnvironmentPolicyMode::Strict
    ) {
        let message = environment_policy_result
            .violations
            .first()
            .map(|violation| {
                format!(
                    "environment policy failed for {}: {}",
                    violation.variable, violation.reason
                )
            })
            .unwrap_or_else(|| "environment policy failed".to_string());
        return persist_failed_capture(
            &run_id,
            &artifact_paths,
            &request,
            created_unix_ms,
            env_sha256,
            FailureClass::EnvironmentPolicyFailed,
            &message,
            environment_policy_result,
        );
    }

    let mut child = Command::new(&request.command.program);
    child.args(&request.command.args);
    child.current_dir(&repo);
    child.stdin(Stdio::null());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::piped());

    let start_instant = Instant::now();
    let mut spawned = match child.spawn() {
        Ok(child) => child,
        Err(error) => {
            let spawn_message = error.to_string();
            return persist_failed_capture(
                &run_id,
                &artifact_paths,
                &request,
                created_unix_ms,
                env_sha256,
                FailureClass::SpawnFailed,
                &spawn_message,
                environment_policy_result,
            );
        }
    };

    let stdout = match spawned.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return persist_failed_capture(
                &run_id,
                &artifact_paths,
                &request,
                created_unix_ms,
                env_sha256,
                FailureClass::SpawnFailed,
                "stdout pipe unavailable",
                environment_policy_result,
            );
        }
    };
    let stderr = match spawned.stderr.take() {
        Some(stderr) => stderr,
        None => {
            return persist_failed_capture(
                &run_id,
                &artifact_paths,
                &request,
                created_unix_ms,
                env_sha256,
                FailureClass::SpawnFailed,
                "stderr pipe unavailable",
                environment_policy_result,
            );
        }
    };

    let shared = Arc::new(Mutex::new(SharedCaptureState::new()));
    let output_limit_hit = Arc::new(AtomicBool::new(false));

    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();

    spawn_reader(ReaderStart {
        stream_name: "stdout",
        reader: stdout,
        path: stdout_path.clone(),
        max_stream_bytes: request.policy.max_stdout_bytes,
        max_combined_bytes: request.policy.max_combined_output_bytes,
        shared: Arc::clone(&shared),
        output_limit_hit: Arc::clone(&output_limit_hit),
        tx: stdout_tx,
    });
    spawn_reader(ReaderStart {
        stream_name: "stderr",
        reader: stderr,
        path: stderr_path.clone(),
        max_stream_bytes: request.policy.max_stderr_bytes,
        max_combined_bytes: request.policy.max_combined_output_bytes,
        shared: Arc::clone(&shared),
        output_limit_hit: Arc::clone(&output_limit_hit),
        tx: stderr_tx,
    });

    let wall_timeout = Duration::from_secs(request.policy.wall_timeout_seconds);
    let drain_deadline = Duration::from_secs(5);
    let mut timed_out = false;
    let mut child_exit_status = None;
    let mut stream_drain_failed = false;
    let mut termination_requested = false;

    loop {
        if let Some(status) = spawned.try_wait()? {
            child_exit_status = Some(status);
            break;
        }

        if output_limit_hit.load(Ordering::SeqCst) {
            let _ = spawned.kill();
            termination_requested = true;
            break;
        }

        if start_instant.elapsed() >= wall_timeout {
            timed_out = true;
            let _ = spawned.kill();
            termination_requested = true;
            break;
        }

        thread::sleep(Duration::from_millis(25));
    }

    let drain_deadline_instant = Instant::now() + drain_deadline;

    if child_exit_status.is_none() {
        loop {
            if let Some(status) = spawned.try_wait()? {
                child_exit_status = Some(status);
                break;
            }
            if Instant::now() >= drain_deadline_instant {
                if timed_out || output_limit_hit.load(Ordering::SeqCst) {
                    stream_drain_failed = true;
                }
                break;
            }
            if termination_requested && output_limit_hit.load(Ordering::SeqCst) {
                let _ = spawned.kill();
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    if timed_out || output_limit_hit.load(Ordering::SeqCst) {
        let _ = spawned.kill();
    }

    let stdout_completion = wait_for_reader(stdout_rx, drain_deadline_instant);
    let stderr_completion = wait_for_reader(stderr_rx, drain_deadline_instant);

    if stdout_completion.is_none() || stderr_completion.is_none() {
        stream_drain_failed = true;
    }

    let stdout_result = stdout_completion
        .map(|c| c.result)
        .unwrap_or_else(|| empty_stream_result(stdout_path.clone()));
    let stderr_result = stderr_completion
        .map(|c| c.result)
        .unwrap_or_else(|| empty_stream_result(stderr_path.clone()));

    let combined_output_bytes = stdout_result.bytes.saturating_add(stderr_result.bytes);
    let runtime_ms = start_instant.elapsed().as_millis();

    let (status, failure_class, exit_code, output_limit_exceeded) = finalize_status(
        timed_out,
        output_limit_hit.load(Ordering::SeqCst),
        stream_drain_failed,
        child_exit_status,
    );

    let command_result = CommandCaptureResult {
        id: request.id.clone(),
        program: request.command.program.clone(),
        args: request.command.args.clone(),
        cwd: repo.to_string_lossy().into_owned(),
        status: status.clone(),
        failure_class,
        exit_code,
        timed_out,
        output_limit_exceeded,
        runtime_ms,
        wall_timeout_seconds: request.policy.wall_timeout_seconds,
        stdout: stdout_result,
        stderr: stderr_result,
        combined_output_bytes,
        max_stdout_bytes: request.policy.max_stdout_bytes,
        max_stderr_bytes: request.policy.max_stderr_bytes,
        max_combined_output_bytes: request.policy.max_combined_output_bytes,
        env_path: env_path.to_string_lossy().into_owned(),
        env_sha256,
        environment_policy: environment_policy_result.clone(),
        result_path: result_path.to_string_lossy().into_owned(),
        command_path: command_path.to_string_lossy().into_owned(),
    };

    write_json_file(&result_path, &command_result)?;

    let entry = CommandEvidenceEntry {
        id: command_result.id.clone(),
        program: command_result.program.clone(),
        args: command_result.args.clone(),
        cwd: command_result.cwd.clone(),
        status: command_result.status.clone(),
        failure_class: command_result.failure_class.clone(),
        exit_code: command_result.exit_code,
        timed_out: command_result.timed_out,
        output_limit_exceeded: command_result.output_limit_exceeded,
        runtime_ms: command_result.runtime_ms,
        result_path: command_result.result_path.clone(),
        command_path: command_result.command_path.clone(),
        stdout_path: command_result.stdout.path.clone(),
        stderr_path: command_result.stderr.path.clone(),
        env_path: command_result.env_path.clone(),
        env_sha256: command_result.env_sha256.clone(),
    };

    let manifest = EvidenceManifest {
        run_id: run_id.clone(),
        repo_path: repo.to_string_lossy().into_owned(),
        created_unix_ms,
        aggregate_status: command_result.status.clone(),
        commands: vec![entry],
    };
    write_json_file(&manifest_path, &manifest)?;

    let run_record = RunRecord {
        run_id,
        repo_path: repo.to_string_lossy().into_owned(),
        created_unix_ms,
        command_ids: vec![request.id],
        run_path: run_dir.to_string_lossy().into_owned(),
        run_json_path: run_json_path.to_string_lossy().into_owned(),
        evidence_manifest_path: manifest_path.to_string_lossy().into_owned(),
    };
    write_json_file(&run_json_path, &run_record)?;

    Ok(CaptureOutcome {
        run: run_record,
        manifest,
        command_result,
    })
}

#[derive(Debug, Serialize)]
struct CommandArtifact {
    id: String,
    repo_path: String,
    cwd: String,
    program: String,
    args: Vec<String>,
    wall_timeout_seconds: u64,
    max_stdout_bytes: u64,
    max_stderr_bytes: u64,
    max_combined_output_bytes: u64,
    on_output_limit: OutputLimitPolicy,
    capture_env: bool,
    environment_policy: EnvironmentPolicy,
    created_unix_ms: u128,
}

struct ReaderStart<R>
where
    R: Read + Send + 'static,
{
    stream_name: &'static str,
    reader: R,
    path: PathBuf,
    max_stream_bytes: u64,
    max_combined_bytes: u64,
    shared: Arc<Mutex<SharedCaptureState>>,
    output_limit_hit: Arc<AtomicBool>,
    tx: mpsc::Sender<ReaderCompletion>,
}

fn spawn_reader<R>(config: ReaderStart<R>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let result = read_stream(
            config.stream_name,
            config.reader,
            &config.path,
            config.max_stream_bytes,
            config.max_combined_bytes,
            config.shared,
            config.output_limit_hit,
        );
        let _ = config.tx.send(ReaderCompletion { result });
    });
}

fn read_stream(
    stream_name: &'static str,
    reader: impl Read,
    path: &Path,
    max_stream_bytes: u64,
    max_combined_bytes: u64,
    shared: Arc<Mutex<SharedCaptureState>>,
    output_limit_hit: Arc<AtomicBool>,
) -> StreamCaptureResult {
    let file = match File::create(path) {
        Ok(file) => file,
        Err(_) => {
            output_limit_hit.store(true, Ordering::SeqCst);
            return empty_stream_result(path.to_path_buf());
        }
    };

    let mut writer = io::BufWriter::new(file);
    let mut reader = BufReader::new(reader);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut local_truncated = false;
    let mut local_bytes = 0u64;

    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(_) => {
                output_limit_hit.store(true, Ordering::SeqCst);
                local_truncated = true;
                break;
            }
        };

        if read == 0 {
            break;
        }

        let saved = {
            let mut guard = shared.lock().expect("capture state mutex poisoned");
            let (current_stream_bytes, current_combined_bytes) = match stream_name {
                "stdout" => (guard.stdout.bytes, guard.combined_output_bytes),
                "stderr" => (guard.stderr.bytes, guard.combined_output_bytes),
                _ => unreachable!(),
            };
            let stream_remaining = max_stream_bytes.saturating_sub(current_stream_bytes);
            let combined_remaining = max_combined_bytes.saturating_sub(current_combined_bytes);
            let saved = usize::try_from(stream_remaining.min(combined_remaining))
                .unwrap_or(usize::MAX)
                .min(read);

            match stream_name {
                "stdout" => {
                    if saved > 0 {
                        guard.stdout.bytes = guard.stdout.bytes.saturating_add(saved as u64);
                        local_bytes = local_bytes.saturating_add(saved as u64);
                    }
                    if saved < read {
                        guard.stdout.truncated = true;
                        guard.output_limit_exceeded = true;
                        local_truncated = true;
                    }
                }
                "stderr" => {
                    if saved > 0 {
                        guard.stderr.bytes = guard.stderr.bytes.saturating_add(saved as u64);
                        local_bytes = local_bytes.saturating_add(saved as u64);
                    }
                    if saved < read {
                        guard.stderr.truncated = true;
                        guard.output_limit_exceeded = true;
                        local_truncated = true;
                    }
                }
                _ => unreachable!(),
            }

            if saved > 0 {
                guard.combined_output_bytes =
                    guard.combined_output_bytes.saturating_add(saved as u64);
            }
            saved
        };

        if saved > 0 {
            if writer.write_all(&buffer[..saved]).is_err() {
                output_limit_hit.store(true, Ordering::SeqCst);
                local_truncated = true;
                break;
            }
            hasher.update(&buffer[..saved]);
        }

        if saved < read {
            output_limit_hit.store(true, Ordering::SeqCst);
        }
    }

    let _ = writer.flush();

    let sha256 = hex_encode(hasher.finalize());
    let complete = !local_truncated;
    StreamCaptureResult {
        path: path.to_string_lossy().into_owned(),
        sha256,
        bytes: local_bytes,
        complete,
        truncated: local_truncated,
        hash_scope: HashScope::SavedBytesOnly,
    }
}

fn wait_for_reader(
    rx: mpsc::Receiver<ReaderCompletion>,
    deadline: Instant,
) -> Option<ReaderCompletion> {
    let now = Instant::now();
    if now >= deadline {
        return None;
    }
    rx.recv_timeout(deadline.saturating_duration_since(now))
        .ok()
}

fn empty_stream_result(path: PathBuf) -> StreamCaptureResult {
    StreamCaptureResult {
        path: path.to_string_lossy().into_owned(),
        sha256: hex_encode(Sha256::new().finalize()),
        bytes: 0,
        complete: false,
        truncated: true,
        hash_scope: HashScope::SavedBytesOnly,
    }
}

fn finalize_status(
    timed_out: bool,
    output_limit_exceeded: bool,
    stream_drain_failed: bool,
    child_exit_status: Option<ExitStatus>,
) -> (CommandStatus, Option<FailureClass>, Option<i32>, bool) {
    if stream_drain_failed {
        return (
            CommandStatus::Fail,
            Some(FailureClass::StreamDrainFailed),
            None,
            output_limit_exceeded,
        );
    }

    if timed_out {
        return (
            CommandStatus::Fail,
            Some(FailureClass::TimeoutExceeded),
            None,
            output_limit_exceeded,
        );
    }

    if output_limit_exceeded {
        return (
            CommandStatus::Fail,
            Some(FailureClass::OutputLimitExceeded),
            None,
            true,
        );
    }

    match child_exit_status.and_then(|status| status.code()) {
        Some(0) => (CommandStatus::Pass, None, Some(0), false),
        Some(code) => (
            CommandStatus::Fail,
            Some(FailureClass::NonZeroExit),
            Some(code),
            false,
        ),
        None => (
            CommandStatus::Fail,
            Some(FailureClass::IoError),
            None,
            false,
        ),
    }
}

fn capture_environment() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for (key, value) in std::env::vars_os() {
        env.insert(
            key.to_string_lossy().into_owned(),
            value.to_string_lossy().into_owned(),
        );
    }
    env
}

fn generate_run_id() -> String {
    let now = system_time_ms(SystemTime::now());
    format!("{}-{}", now, std::process::id())
}

fn system_time_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
}

fn sanitize_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "capture".to_string()
    } else {
        out
    }
}

fn write_json_file<P: AsRef<Path>, T: Serialize>(path: P, value: &T) -> Result<(), CaptureError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ArtifactPaths {
    run_dir: PathBuf,
    command_path: PathBuf,
    env_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    result_path: PathBuf,
    run_json_path: PathBuf,
    manifest_path: PathBuf,
}

#[allow(clippy::too_many_arguments)]
fn persist_failed_capture(
    run_id: &str,
    paths: &ArtifactPaths,
    request: &CaptureRequest,
    created_unix_ms: u128,
    env_sha256: String,
    failure_class: FailureClass,
    message: &str,
    environment_policy: EnvironmentPolicyResult,
) -> Result<CaptureOutcome, CaptureError> {
    fs::write(&paths.stdout_path, b"")?;
    fs::write(&paths.stderr_path, message.as_bytes())?;

    let stdout_sha256 = sha256_hex(b"");
    let stderr_sha256 = sha256_hex(message.as_bytes());
    let stderr_bytes = message.len() as u64;

    let command_result = CommandCaptureResult {
        id: request.id.clone(),
        program: request.command.program.clone(),
        args: request.command.args.clone(),
        cwd: request
            .repo
            .canonicalize()
            .unwrap_or_else(|_| request.repo.clone())
            .to_string_lossy()
            .into_owned(),
        status: CommandStatus::Fail,
        failure_class: Some(failure_class),
        exit_code: None,
        timed_out: false,
        output_limit_exceeded: false,
        runtime_ms: 0,
        wall_timeout_seconds: request.policy.wall_timeout_seconds,
        stdout: StreamCaptureResult {
            path: paths.stdout_path.to_string_lossy().into_owned(),
            sha256: stdout_sha256.clone(),
            bytes: 0,
            complete: true,
            truncated: false,
            hash_scope: HashScope::SavedBytesOnly,
        },
        stderr: StreamCaptureResult {
            path: paths.stderr_path.to_string_lossy().into_owned(),
            sha256: stderr_sha256.clone(),
            bytes: stderr_bytes,
            complete: true,
            truncated: false,
            hash_scope: HashScope::SavedBytesOnly,
        },
        combined_output_bytes: stderr_bytes,
        max_stdout_bytes: request.policy.max_stdout_bytes,
        max_stderr_bytes: request.policy.max_stderr_bytes,
        max_combined_output_bytes: request.policy.max_combined_output_bytes,
        env_path: paths.env_path.to_string_lossy().into_owned(),
        env_sha256,
        environment_policy,
        result_path: paths.result_path.to_string_lossy().into_owned(),
        command_path: paths.command_path.to_string_lossy().into_owned(),
    };

    write_json_file(&paths.result_path, &command_result)?;

    let manifest = EvidenceManifest {
        run_id: run_id.to_string(),
        repo_path: request
            .repo
            .canonicalize()
            .unwrap_or_else(|_| request.repo.clone())
            .to_string_lossy()
            .into_owned(),
        created_unix_ms,
        aggregate_status: CommandStatus::Fail,
        commands: vec![CommandEvidenceEntry {
            id: command_result.id.clone(),
            program: command_result.program.clone(),
            args: command_result.args.clone(),
            cwd: command_result.cwd.clone(),
            status: command_result.status.clone(),
            failure_class: command_result.failure_class.clone(),
            exit_code: command_result.exit_code,
            timed_out: command_result.timed_out,
            output_limit_exceeded: command_result.output_limit_exceeded,
            runtime_ms: command_result.runtime_ms,
            result_path: command_result.result_path.clone(),
            command_path: command_result.command_path.clone(),
            stdout_path: command_result.stdout.path.clone(),
            stderr_path: command_result.stderr.path.clone(),
            env_path: command_result.env_path.clone(),
            env_sha256: command_result.env_sha256.clone(),
        }],
    };
    write_json_file(&paths.manifest_path, &manifest)?;

    let run_record = RunRecord {
        run_id: manifest.run_id.clone(),
        repo_path: manifest.repo_path.clone(),
        created_unix_ms,
        command_ids: vec![request.id.clone()],
        run_path: paths.run_dir.to_string_lossy().into_owned(),
        run_json_path: paths.run_json_path.to_string_lossy().into_owned(),
        evidence_manifest_path: paths.manifest_path.to_string_lossy().into_owned(),
    };
    write_json_file(&paths.run_json_path, &run_record)?;

    Ok(CaptureOutcome {
        run: run_record,
        manifest,
        command_result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::EnvironmentPolicyStatus;
    use crate::evidence::{CapturePolicy, CommandSpec};
    use std::sync::OnceLock;

    fn repo_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ccl_capture_repo_{}_{}",
            std::process::id(),
            system_time_ms(SystemTime::now())
        ));
        fs::create_dir_all(dir.join(".git")).unwrap();
        dir
    }

    fn helper_exe() -> PathBuf {
        static HELPER: OnceLock<PathBuf> = OnceLock::new();
        HELPER
            .get_or_init(|| {
                let root = std::env::temp_dir().join(format!(
                    "ccl_capture_helper_{}_{}",
                    std::process::id(),
                    system_time_ms(SystemTime::now())
                ));
                fs::create_dir_all(&root).unwrap();
                let src = root.join("helper.rs");
                let exe = root.join(format!("helper{}", if cfg!(windows) { ".exe" } else { "" }));
                fs::write(
                    &src,
                    r#"
use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "pass" => {
            println!("stdout ok");
            eprintln!("stderr ok");
        }
        "nonzero" => {
            eprintln!("exit 7");
            std::process::exit(7);
        }
        "sleep" => {
            thread::sleep(Duration::from_secs(5));
        }
        "spam" => {
            let out = vec![b'A'; 4096];
            let err = vec![b'B'; 4096];
            let t1 = thread::spawn(move || {
                let mut stdout = io::stdout().lock();
                for _ in 0..256 {
                    stdout.write_all(&out).unwrap();
                    stdout.flush().unwrap();
                }
            });
            let t2 = thread::spawn(move || {
                let mut stderr = io::stderr().lock();
                for _ in 0..256 {
                    stderr.write_all(&err).unwrap();
                    stderr.flush().unwrap();
                }
            });
            t1.join().unwrap();
            t2.join().unwrap();
        }
        "limit" => {
            let out = vec![b'O'; 4096];
            let err = vec![b'E'; 4096];
            let mut stdout = io::stdout().lock();
            let mut stderr = io::stderr().lock();
            for _ in 0..512 {
                stdout.write_all(&out).unwrap();
                stdout.flush().unwrap();
                stderr.write_all(&err).unwrap();
                stderr.flush().unwrap();
            }
        }
        "limit_forever" => {
            let out = vec![b'O'; 4096];
            let err = vec![b'E'; 4096];
            let t1 = thread::spawn(move || {
                let mut stdout = io::stdout().lock();
                loop {
                    stdout.write_all(&out).unwrap();
                    stdout.flush().unwrap();
                }
            });
            let t2 = thread::spawn(move || {
                let mut stderr = io::stderr().lock();
                loop {
                    stderr.write_all(&err).unwrap();
                    stderr.flush().unwrap();
                }
            });
            t1.join().unwrap();
            t2.join().unwrap();
        }
        _ => {
            println!("default");
        }
    }
}
"#,
                )
                .unwrap();
                let status = Command::new("rustc")
                    .arg(&src)
                    .arg("-O")
                    .arg("-o")
                    .arg(&exe)
                    .status()
                    .unwrap();
                assert!(status.success());
                exe
            })
            .clone()
    }

    fn capture(repo: &Path, id: &str, args: &[&str], policy: CapturePolicy) -> CaptureOutcome {
        capture_command(CaptureRequest {
            id: id.to_string(),
            repo: repo.to_path_buf(),
            command: CommandSpec {
                program: helper_exe().to_string_lossy().into_owned(),
                args: args.iter().map(|s| s.to_string()).collect(),
            },
            policy,
            environment_policy: None,
        })
        .unwrap()
    }

    #[test]
    fn successful_command_capture_writes_artifacts_and_hashes() {
        let repo = repo_dir();
        let outcome = capture(&repo, "success", &["pass"], CapturePolicy::default());

        assert_eq!(outcome.command_result.status, CommandStatus::Pass);
        assert!(Path::new(&outcome.command_result.stdout.path).exists());
        assert!(Path::new(&outcome.command_result.stderr.path).exists());
        assert!(Path::new(&outcome.command_result.result_path).exists());
        assert!(Path::new(&outcome.manifest.commands[0].result_path).exists());
        assert!(outcome.command_result.stdout.bytes > 0);
        assert!(outcome.command_result.stderr.bytes > 0);
        assert_eq!(
            file_sha256(Path::new(&outcome.command_result.stdout.path)).unwrap(),
            outcome.command_result.stdout.sha256
        );
        assert_eq!(
            file_sha256(Path::new(&outcome.command_result.stderr.path)).unwrap(),
            outcome.command_result.stderr.sha256
        );
        assert!(Path::new(&outcome.command_result.env_path).exists());
        assert!(!outcome.command_result.env_sha256.is_empty());
        assert_eq!(
            outcome.command_result.environment_policy.status,
            EnvironmentPolicyStatus::Pass
        );
    }

    #[test]
    fn non_zero_exit_is_captured() {
        let repo = repo_dir();
        let outcome = capture(&repo, "fail", &["nonzero"], CapturePolicy::default());

        assert_eq!(outcome.command_result.status, CommandStatus::Fail);
        assert_eq!(
            outcome.command_result.failure_class,
            Some(FailureClass::NonZeroExit)
        );
        assert_eq!(outcome.command_result.exit_code, Some(7));
    }

    #[test]
    fn output_limit_is_recorded_and_truncated() {
        let repo = repo_dir();
        let policy = CapturePolicy {
            max_stdout_bytes: 32 * 1024,
            max_stderr_bytes: 32 * 1024,
            max_combined_output_bytes: 48 * 1024,
            ..CapturePolicy::default()
        };

        let outcome = capture(&repo, "limit", &["limit_forever"], policy);

        assert_eq!(outcome.command_result.status, CommandStatus::Fail);
        assert_eq!(
            outcome.command_result.failure_class,
            Some(FailureClass::OutputLimitExceeded)
        );
        assert!(outcome.command_result.output_limit_exceeded);
        assert!(!outcome.command_result.timed_out);
        assert!(outcome.command_result.runtime_ms < 10_000);
        assert!(outcome.command_result.stdout.truncated || outcome.command_result.stderr.truncated);
        assert_eq!(
            file_sha256(Path::new(&outcome.command_result.stdout.path)).unwrap(),
            outcome.command_result.stdout.sha256
        );
        assert_eq!(
            file_sha256(Path::new(&outcome.command_result.stderr.path)).unwrap(),
            outcome.command_result.stderr.sha256
        );
    }

    #[test]
    fn spawn_failure_persists_artifacts() {
        let repo = repo_dir();
        let outcome = capture_command(CaptureRequest {
            id: "spawn-failure".to_string(),
            repo: repo.clone(),
            command: CommandSpec {
                program: "definitely-not-a-real-binary-12345".to_string(),
                args: vec![],
            },
            policy: CapturePolicy::default(),
            environment_policy: None,
        })
        .unwrap();

        assert_eq!(outcome.command_result.status, CommandStatus::Fail);
        assert_eq!(
            outcome.command_result.failure_class,
            Some(FailureClass::SpawnFailed)
        );
        assert_eq!(outcome.command_result.exit_code, None);
        assert!(Path::new(&outcome.command_result.result_path).exists());
        assert!(Path::new(&outcome.run.evidence_manifest_path).exists());
        assert!(Path::new(&outcome.run.run_json_path).exists());
        assert!(Path::new(&outcome.command_result.stderr.path).exists());
    }

    #[test]
    fn timeout_is_captured() {
        let repo = repo_dir();
        let policy = CapturePolicy {
            wall_timeout_seconds: 1,
            ..CapturePolicy::default()
        };

        let outcome = capture(&repo, "timeout", &["sleep"], policy);

        assert_eq!(outcome.command_result.status, CommandStatus::Fail);
        assert_eq!(
            outcome.command_result.failure_class,
            Some(FailureClass::TimeoutExceeded)
        );
        assert!(outcome.command_result.timed_out);
        assert_eq!(outcome.command_result.exit_code, None);
    }

    #[test]
    fn streams_are_read_concurrently_enough_for_pipe_backpressure() {
        let repo = repo_dir();
        let outcome = capture(&repo, "spam", &["spam"], CapturePolicy::default());

        assert_eq!(outcome.command_result.status, CommandStatus::Pass);
        assert!(outcome.command_result.stdout.bytes >= 1024 * 1024);
        assert!(outcome.command_result.stderr.bytes >= 1024 * 1024);
    }

    #[test]
    fn evidence_manifest_and_run_record_are_created() {
        let repo = repo_dir();
        let outcome = capture(&repo, "manifest", &["pass"], CapturePolicy::default());

        assert!(Path::new(&outcome.run.run_json_path).exists());
        assert!(Path::new(&outcome.run.evidence_manifest_path).exists());
        assert_eq!(outcome.manifest.run_id, outcome.run.run_id);
        assert_eq!(outcome.manifest.commands.len(), 1);
    }

    #[test]
    fn env_snapshot_exists_and_is_hashed() {
        let repo = repo_dir();
        let outcome = capture(&repo, "env", &["pass"], CapturePolicy::default());

        assert!(Path::new(&outcome.command_result.env_path).exists());
        let bytes = fs::read(&outcome.command_result.env_path).unwrap();
        assert_eq!(sha256_hex(&bytes), outcome.command_result.env_sha256);
    }

    fn file_sha256(path: &Path) -> Result<String, io::Error> {
        let bytes = fs::read(path)?;
        Ok(sha256_hex(&bytes))
    }
}
