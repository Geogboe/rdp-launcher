use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value, json};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::store::AppPaths;

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

static GLOBAL_LOGGER: OnceLock<FileLogger> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

pub struct FileLogger {
    path: PathBuf,
    surface: String,
    state: Mutex<LoggerState>,
}

struct LoggerState {
    file: Option<File>,
    bytes_written: u64,
}

impl FileLogger {
    pub fn create(paths: &AppPaths, surface: impl Into<String>) -> Result<Self, LoggingError> {
        fs::create_dir_all(&paths.logs_dir).map_err(LoggingError::CreateDir)?;
        rotate_on_startup_if_needed(&paths.app_log)?;
        let state = open_logger_state(&paths.app_log)?;

        Ok(Self {
            path: paths.app_log.clone(),
            surface: surface.into(),
            state: Mutex::new(state),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn log(
        &self,
        level: LogLevel,
        event: &str,
        message: &str,
        fields: Value,
    ) -> Result<(), LoggingError> {
        let mut record = Map::new();
        record.insert(
            "timestamp".to_owned(),
            Value::String(
                OffsetDateTime::now_utc()
                    .format(&Rfc3339)
                    .map_err(LoggingError::Time)?,
            ),
        );
        record.insert("level".to_owned(), Value::String(level.as_str().to_owned()));
        record.insert("event".to_owned(), Value::String(event.to_owned()));
        record.insert("message".to_owned(), Value::String(message.to_owned()));
        record.insert("surface".to_owned(), Value::String(self.surface.clone()));
        record.insert("pid".to_owned(), json!(std::process::id()));
        record.insert(
            "fields".to_owned(),
            if fields.is_object() {
                fields
            } else {
                json!({})
            },
        );

        let mut line = serde_json::to_vec(&Value::Object(record)).map_err(LoggingError::Json)?;
        line.push(b'\n');

        let mut state = self.state.lock().map_err(|_| LoggingError::Poisoned)?;
        rotate_if_needed(&self.path, &mut state, line.len() as u64)?;
        let file = state.file.as_mut().ok_or(LoggingError::LoggerClosed)?;
        file.write_all(&line).map_err(LoggingError::Write)?;
        file.flush().map_err(LoggingError::Write)?;
        state.bytes_written += line.len() as u64;
        Ok(())
    }
}

pub fn init_global_logger(
    paths: &AppPaths,
    surface: impl Into<String>,
) -> Result<PathBuf, LoggingError> {
    if let Some(existing) = GLOBAL_LOGGER.get() {
        return Ok(existing.path().to_path_buf());
    }

    let logger = FileLogger::create(paths, surface)?;
    let path = logger.path().to_path_buf();
    match GLOBAL_LOGGER.set(logger) {
        Ok(()) => {}
        Err(_) => {
            return Ok(GLOBAL_LOGGER
                .get()
                .expect("logger initialized")
                .path()
                .to_path_buf());
        }
    }

    info(
        "logging.initialized",
        "file logging initialized",
        json!({ "path": path.display().to_string() }),
    );

    Ok(path)
}

pub fn info(event: &str, message: &str, fields: Value) {
    let _ = log_global(LogLevel::Info, event, message, fields);
}

pub fn warn(event: &str, message: &str, fields: Value) {
    let _ = log_global(LogLevel::Warn, event, message, fields);
}

pub fn error(event: &str, message: &str, fields: Value) {
    let _ = log_global(LogLevel::Error, event, message, fields);
}

pub fn debug(event: &str, message: &str, fields: Value) {
    let _ = log_global(LogLevel::Debug, event, message, fields);
}

fn log_global(
    level: LogLevel,
    event: &str,
    message: &str,
    fields: Value,
) -> Result<(), LoggingError> {
    let Some(logger) = GLOBAL_LOGGER.get() else {
        return Ok(());
    };
    logger.log(level, event, message, fields)
}

fn open_logger_state(path: &Path) -> Result<LoggerState, LoggingError> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(LoggingError::Open)?;
    let bytes_written = file.metadata().map_err(LoggingError::Metadata)?.len();
    Ok(LoggerState {
        file: Some(file),
        bytes_written,
    })
}

fn rotate_on_startup_if_needed(path: &Path) -> Result<(), LoggingError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(LoggingError::Metadata(error)),
    };

    if metadata.len() < MAX_LOG_BYTES {
        return Ok(());
    }

    rotate_log_file(path)
}

fn rotate_if_needed(
    path: &Path,
    state: &mut LoggerState,
    next_line_bytes: u64,
) -> Result<(), LoggingError> {
    if state.bytes_written == 0 || state.bytes_written + next_line_bytes <= MAX_LOG_BYTES {
        return Ok(());
    }

    if let Some(file) = state.file.as_mut() {
        file.flush().map_err(LoggingError::Write)?;
    }
    let _ = state.file.take();
    rotate_log_file(path)?;
    *state = open_logger_state(path)?;
    Ok(())
}

fn rotate_log_file(path: &Path) -> Result<(), LoggingError> {
    let rotated_path = path.with_extension("log.1");
    match fs::remove_file(&rotated_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(LoggingError::Rotate(error)),
    }

    fs::rename(path, &rotated_path).map_err(LoggingError::Rotate)
}

#[derive(Debug, Error)]
pub enum LoggingError {
    #[error("failed to create log directory: {0}")]
    CreateDir(std::io::Error),
    #[error("failed to open log file: {0}")]
    Open(std::io::Error),
    #[error("failed to write log file: {0}")]
    Write(std::io::Error),
    #[error("failed to inspect log file metadata: {0}")]
    Metadata(std::io::Error),
    #[error("failed to rotate log file: {0}")]
    Rotate(std::io::Error),
    #[error("failed to format log timestamp: {0}")]
    Time(time::error::Format),
    #[error("failed to encode log record: {0}")]
    Json(serde_json::Error),
    #[error("logger state was poisoned")]
    Poisoned,
    #[error("logger file handle is unexpectedly unavailable")]
    LoggerClosed,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use super::*;

    #[test]
    fn file_logger_writes_json_line() {
        let root = std::env::temp_dir().join(format!("rdp-launch-log-test-{}", Uuid::now_v7()));
        let paths = AppPaths::from_root(&root);
        let logger = FileLogger::create(&paths, "test").expect("logger");

        logger
            .log(
                LogLevel::Info,
                "test.event",
                "test message",
                json!({ "profile_id": "abc123" }),
            )
            .expect("log");

        let contents = fs::read_to_string(&paths.app_log).expect("log file");
        assert!(contents.contains("\"event\":\"test.event\""));
        assert!(contents.contains("\"surface\":\"test\""));
        assert!(contents.contains("\"profile_id\":\"abc123\""));

        fs::remove_dir_all(root).expect("cleanup temp log dir");
    }

    #[test]
    fn file_logger_rotates_large_existing_log_on_startup() {
        let root = std::env::temp_dir().join(format!("rdp-launch-log-test-{}", Uuid::now_v7()));
        let paths = AppPaths::from_root(&root);
        fs::create_dir_all(&paths.logs_dir).expect("logs dir");
        fs::write(&paths.app_log, vec![b'x'; (MAX_LOG_BYTES as usize) + 16]).expect("seed log");

        let logger = FileLogger::create(&paths, "test").expect("logger");
        logger
            .log(LogLevel::Info, "test.event", "test message", json!({}))
            .expect("log");

        let rotated = paths.app_log.with_extension("log.1");
        assert!(rotated.exists());
        let live_size = fs::metadata(&paths.app_log).expect("live metadata").len();
        let rotated_size = fs::metadata(&rotated).expect("rotated metadata").len();
        assert!(live_size < MAX_LOG_BYTES);
        assert!(rotated_size >= MAX_LOG_BYTES);

        fs::remove_dir_all(root).expect("cleanup temp log dir");
    }
}
