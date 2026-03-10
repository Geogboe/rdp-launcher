use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::logging;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperConfig {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperRequestEnvelope<T> {
    pub version: u8,
    pub request_id: String,
    pub op: String,
    pub sent_at: OffsetDateTime,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperResponseEnvelope<T> {
    pub version: u8,
    pub request_id: String,
    pub ok: bool,
    pub payload: Option<T>,
    pub error: Option<HelperErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperProbe {
    pub helper_name: String,
    pub helper_version: String,
    pub supports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolveResult {
    Resolved,
    Cancelled,
    Denied,
    Prompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperResolve {
    pub result: ResolveResult,
    pub credentials: Option<ResolveCredentials>,
    pub ttl_seconds: Option<u64>,
    pub display_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvePayload {
    pub profile: HelperProfileRef,
    pub target: HelperTargetRef,
    pub preset: Option<HelperPresetRef>,
    pub requested_fields: Vec<String>,
    pub launch_context: HelperLaunchContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperProfileRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperTargetRef {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperPresetRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperLaunchContext {
    pub surface: String,
    pub reason: String,
    pub allow_windows_vault_bridge: bool,
}

#[derive(Debug, Clone)]
pub struct HelperClient {
    config: HelperConfig,
}

impl HelperClient {
    pub fn new(config: HelperConfig) -> Self {
        Self { config }
    }

    pub fn probe(&self) -> Result<HelperProbe, HelperError> {
        let request = HelperRequestEnvelope {
            version: 1,
            request_id: Uuid::now_v7().to_string(),
            op: "probe".to_owned(),
            sent_at: OffsetDateTime::now_utc(),
            payload: serde_json::json!({}),
        };
        let response: HelperResponseEnvelope<HelperProbe> =
            self.exec(request, Duration::from_secs(3))?;
        let probe = Self::extract_ok(response)?;
        logging::info(
            "helper.probe.succeeded",
            "helper probe completed",
            serde_json::json!({
                "executable": &self.config.executable,
                "helper_name": &probe.helper_name,
                "helper_version": &probe.helper_version,
            }),
        );
        Ok(probe)
    }

    pub fn resolve(&self, payload: ResolvePayload) -> Result<HelperResolve, HelperError> {
        let request = HelperRequestEnvelope {
            version: 1,
            request_id: Uuid::now_v7().to_string(),
            op: "resolve".to_owned(),
            sent_at: OffsetDateTime::now_utc(),
            payload,
        };
        let response: HelperResponseEnvelope<HelperResolve> =
            self.exec(request, Duration::from_secs(10))?;
        let resolved = Self::extract_ok(response)?;
        logging::info(
            "helper.resolve.completed",
            "helper resolve completed",
            serde_json::json!({
                "executable": &self.config.executable,
                "result": match &resolved.result {
                    ResolveResult::Resolved => "resolved",
                    ResolveResult::Cancelled => "cancelled",
                    ResolveResult::Denied => "denied",
                    ResolveResult::Prompt => "prompt",
                },
                "has_username": resolved.credentials.as_ref().and_then(|credentials| credentials.username.as_ref()).is_some(),
                "has_password": resolved.credentials.as_ref().and_then(|credentials| credentials.password.as_ref()).is_some(),
                "has_domain": resolved.credentials.as_ref().and_then(|credentials| credentials.domain.as_ref()).is_some(),
            }),
        );
        Ok(resolved)
    }

    fn extract_ok<T>(response: HelperResponseEnvelope<T>) -> Result<T, HelperError> {
        if response.ok {
            response.payload.ok_or(HelperError::MissingPayload)
        } else {
            let error = response.error.ok_or(HelperError::MissingErrorBody)?;
            Err(HelperError::Protocol(error.code, error.message))
        }
    }

    fn exec<Req, Res>(
        &self,
        request: HelperRequestEnvelope<Req>,
        timeout: Duration,
    ) -> Result<HelperResponseEnvelope<Res>, HelperError>
    where
        Req: Serialize,
        Res: for<'de> Deserialize<'de>,
    {
        let request_id = request.request_id.clone();
        let op = request.op.clone();
        let resolved_executable =
            resolve_executable_path(&self.config.executable).inspect_err(|error| {
                logging::error(
                    "helper.exec.invalid_executable",
                    "helper executable failed preflight validation",
                    serde_json::json!({
                        "executable": &self.config.executable,
                        "error": error.to_string(),
                    }),
                );
            })?;
        logging::info(
            "helper.exec.started",
            "starting helper process",
            serde_json::json!({
                "executable": &self.config.executable,
                "resolved_executable": resolved_executable.display().to_string(),
                "args_len": self.config.args.len(),
                "op": &op,
                "request_id": &request_id,
                "timeout_ms": timeout.as_millis(),
            }),
        );

        let mut child = Command::new(&resolved_executable)
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                logging::error(
                    "helper.exec.spawn_failed",
                    "failed to spawn helper process",
                    serde_json::json!({
                        "executable": &self.config.executable,
                        "resolved_executable": resolved_executable.display().to_string(),
                        "error": error.to_string(),
                    }),
                );
                HelperError::Spawn(error)
            })?;

        let serialized = serde_json::to_vec(&request).map_err(HelperError::Json)?;
        let stdin = child.stdin.as_mut().ok_or(HelperError::MissingStdin)?;
        stdin.write_all(&serialized).map_err(HelperError::Io)?;
        stdin.flush().map_err(HelperError::Io)?;
        drop(child.stdin.take());

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > timeout {
                let _ = child.kill();
                logging::warn(
                    "helper.exec.timed_out",
                    "helper process timed out",
                    serde_json::json!({
                        "executable": &self.config.executable,
                        "op": &op,
                        "request_id": &request_id,
                        "elapsed_ms": start.elapsed().as_millis(),
                    }),
                );
                return Err(HelperError::Timeout);
            }

            match child.try_wait().map_err(HelperError::Io)? {
                Some(status) => {
                    let mut stdout = Vec::new();
                    if let Some(mut handle) = child.stdout.take() {
                        handle.read_to_end(&mut stdout).map_err(HelperError::Io)?;
                    }
                    let mut stderr = Vec::new();
                    if let Some(mut handle) = child.stderr.take() {
                        handle.read_to_end(&mut stderr).map_err(HelperError::Io)?;
                    }
                    if !status.success() {
                        logging::error(
                            "helper.exec.non_zero_exit",
                            "helper process exited unsuccessfully",
                            serde_json::json!({
                                "executable": &self.config.executable,
                                "op": &op,
                                "request_id": &request_id,
                                "status_code": status.code(),
                                "stdout_bytes": stdout.len(),
                                "stderr_bytes": stderr.len(),
                                "elapsed_ms": start.elapsed().as_millis(),
                            }),
                        );
                        return Err(HelperError::NonZeroExit(status.code()));
                    }
                    logging::debug(
                        "helper.exec.completed",
                        "helper process completed",
                        serde_json::json!({
                            "executable": &self.config.executable,
                            "op": &op,
                            "request_id": &request_id,
                            "stdout_bytes": stdout.len(),
                            "stderr_bytes": stderr.len(),
                            "elapsed_ms": start.elapsed().as_millis(),
                        }),
                    );
                    return serde_json::from_slice(&stdout).map_err(|error| {
                        logging::error(
                            "helper.exec.invalid_json",
                            "helper returned invalid json",
                            serde_json::json!({
                                "executable": &self.config.executable,
                                "op": &op,
                                "request_id": &request_id,
                                "stdout_bytes": stdout.len(),
                                "stderr_bytes": stderr.len(),
                                "error": error.to_string(),
                            }),
                        );
                        HelperError::Json(error)
                    });
                }
                None => std::thread::sleep(Duration::from_millis(25)),
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("failed to spawn helper: {0}")]
    Spawn(std::io::Error),
    #[error("helper i/o failure: {0}")]
    Io(std::io::Error),
    #[error("failed to resolve helper executable path: {0}")]
    ResolveExecutable(std::io::Error),
    #[error("helper executable was not found: {0}")]
    ExecutableNotFound(String),
    #[error("helper executable path is not a file: {0}")]
    ExecutableNotFile(PathBuf),
    #[error("helper executable is not runnable: {0}")]
    ExecutableNotRunnable(PathBuf),
    #[error("helper json failure: {0}")]
    Json(serde_json::Error),
    #[error("helper process exited with {0:?}")]
    NonZeroExit(Option<i32>),
    #[error("helper timed out")]
    Timeout,
    #[error("helper protocol error {0}: {1}")]
    Protocol(String, String),
    #[error("helper did not expose stdin")]
    MissingStdin,
    #[error("helper response missing payload")]
    MissingPayload,
    #[error("helper error response missing error body")]
    MissingErrorBody,
}

fn resolve_executable_path(executable: &str) -> Result<PathBuf, HelperError> {
    let candidate = Path::new(executable);
    if candidate.is_absolute() || candidate.components().count() > 1 {
        let explicit_path = absolute_path(candidate)?;
        return validate_explicit_executable(explicit_path);
    }

    let Some(path_entries) = env::var_os("PATH") else {
        return Err(HelperError::ExecutableNotFound(executable.to_owned()));
    };

    for directory in env::split_paths(&path_entries) {
        for candidate_path in executable_candidates(directory.join(candidate)) {
            if let Some(resolved) = validate_discovered_executable(&candidate_path)? {
                return Ok(resolved);
            }
        }
    }

    Err(HelperError::ExecutableNotFound(executable.to_owned()))
}

fn absolute_path(path: &Path) -> Result<PathBuf, HelperError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(HelperError::ResolveExecutable)?
            .join(path))
    }
}

fn validate_explicit_executable(path: PathBuf) -> Result<PathBuf, HelperError> {
    match validate_discovered_executable(&path)? {
        Some(resolved) => Ok(resolved),
        None => Err(HelperError::ExecutableNotFound(path.display().to_string())),
    }
}

fn validate_discovered_executable(path: &Path) -> Result<Option<PathBuf>, HelperError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(HelperError::ResolveExecutable(error)),
    };

    if !metadata.is_file() {
        return Err(HelperError::ExecutableNotFile(path.to_path_buf()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(HelperError::ExecutableNotRunnable(path.to_path_buf()));
        }
    }

    Ok(Some(
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
    ))
}

#[cfg(windows)]
fn executable_candidates(path: PathBuf) -> Vec<PathBuf> {
    if path.extension().is_some() {
        return vec![path];
    }

    let mut candidates = vec![path.clone()];
    let extensions =
        env::var_os("PATHEXT").unwrap_or_else(|| std::ffi::OsString::from(".COM;.EXE;.BAT;.CMD"));
    for extension in extensions.to_string_lossy().split(';') {
        let extension = extension.trim();
        if extension.is_empty() {
            continue;
        }
        candidates.push(path.with_extension(extension.trim_start_matches('.')));
    }
    candidates
}

#[cfg(not(windows))]
fn executable_candidates(path: PathBuf) -> Vec<PathBuf> {
    vec![path]
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use uuid::Uuid;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rdp-launch-helper-test-{name}-{}", Uuid::now_v7()))
    }

    #[cfg(unix)]
    #[test]
    fn helper_probe_completes_when_helper_reads_until_eof() {
        let root = temp_dir("probe-eof");
        fs::create_dir_all(&root).expect("temp dir");
        let script_path = root.join("helper.sh");
        fs::write(
            &script_path,
            "#!/bin/sh\nrequest=$(cat)\nif [ -z \"$request\" ]; then exit 9; fi\nprintf '%s' '{\"version\":1,\"request_id\":\"ignored\",\"ok\":true,\"payload\":{\"helper_name\":\"fixture\",\"helper_version\":\"1.0.0\",\"supports\":[\"probe\"]}}'\n",
        )
        .expect("script");

        let mut permissions = fs::metadata(&script_path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("permissions");

        let client = HelperClient::new(HelperConfig {
            executable: script_path.display().to_string(),
            args: Vec::new(),
        });
        let probe = client.probe().expect("probe");

        assert_eq!(probe.helper_name, "fixture");
        assert_eq!(probe.helper_version, "1.0.0");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_executable_rejects_non_executable_files() {
        let root = temp_dir("non-executable");
        fs::create_dir_all(&root).expect("temp dir");
        let script_path = root.join("helper.sh");
        let mut file = fs::File::create(&script_path).expect("script");
        writeln!(file, "#!/bin/sh").expect("script line");
        writeln!(file, "exit 0").expect("script line");

        let mut permissions = fs::metadata(&script_path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&script_path, permissions).expect("permissions");

        let error = resolve_executable_path(script_path.to_str().expect("path string"))
            .expect_err("non executable should fail");
        assert!(matches!(error, HelperError::ExecutableNotRunnable(_)));

        fs::remove_dir_all(root).expect("cleanup");
    }
}
