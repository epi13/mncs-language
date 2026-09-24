//! Generic bounded explicit-argv process capability.
//!
//! This module is part of the language runtime model so source-level process
//! effects and the external `mncs process` adapter share one mechanism. It
//! deliberately contains no application policy: callers provide an explicit
//! executable and argv, a bounded environment and I/O budget, and a deadline.
//! There is no shell path and no ambient environment inheritance.

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const MAX_ARGUMENTS: usize = 128;
pub const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
pub const MAX_ENVIRONMENT: usize = 128;
pub const MAX_ENVIRONMENT_BYTES: usize = 64 * 1024;
pub const MAX_STDIN_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DEADLINE_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRequest {
    pub program: String,
    pub argv: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub stdin: Vec<u8>,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub deadline_ms: u64,
}

impl ProcessRequest {
    pub fn new(program: impl Into<String>, argv: Vec<String>) -> Self {
        Self {
            program: program.into(),
            argv,
            current_dir: None,
            environment: BTreeMap::new(),
            stdin: Vec::new(),
            stdout_limit: MAX_CAPTURE_BYTES,
            stderr_limit: MAX_CAPTURE_BYTES,
            deadline_ms: 60_000,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ProcessError> {
        if self.program.trim().is_empty() {
            return Err(ProcessError::Invalid(
                "program must not be empty".to_owned(),
            ));
        }
        if self.argv.len() > MAX_ARGUMENTS {
            return Err(ProcessError::Limit("too many argv entries".to_owned()));
        }
        if self.argv.iter().any(|arg| arg.len() > MAX_ARGUMENT_BYTES) {
            return Err(ProcessError::Limit(
                "argv entry exceeds byte bound".to_owned(),
            ));
        }
        if self.environment.len() > MAX_ENVIRONMENT
            || self
                .environment
                .keys()
                .any(|key| key.is_empty() || key.contains('='))
            || self
                .environment
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>()
                > MAX_ENVIRONMENT_BYTES
        {
            return Err(ProcessError::Limit("environment exceeds bound".to_owned()));
        }
        if self.stdin.len() > MAX_STDIN_BYTES {
            return Err(ProcessError::Limit("stdin exceeds byte bound".to_owned()));
        }
        if self.stdout_limit > MAX_CAPTURE_BYTES || self.stderr_limit > MAX_CAPTURE_BYTES {
            return Err(ProcessError::Limit(
                "capture limit exceeds bound".to_owned(),
            ));
        }
        if self.deadline_ms == 0 || self.deadline_ms > MAX_DEADLINE_MS {
            return Err(ProcessError::Limit("deadline exceeds bound".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    Invalid(String),
    Limit(String),
    Spawn(String),
    Io(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid process request: {message}"),
            Self::Limit(message) => write!(f, "process request exceeds bound: {message}"),
            Self::Spawn(message) => write!(f, "process spawn failed: {message}"),
            Self::Io(message) => write!(f, "process I/O failed: {message}"),
        }
    }
}

impl std::error::Error for ProcessError {}

pub fn run_bounded(request: &ProcessRequest) -> Result<ProcessResult, ProcessError> {
    request.validate()?;
    let started = Instant::now();
    let mut command = Command::new(&request.program);
    command
        .args(&request.argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = &request.current_dir {
        command.current_dir(directory);
    }
    command.env_clear().envs(&request.environment);
    let mut child = command
        .spawn()
        .map_err(|error| ProcessError::Spawn(error.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        let bytes = request.stdin.clone();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProcessError::Io("stdout pipe unavailable".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ProcessError::Io("stderr pipe unavailable".to_owned()))?;
    let stdout_limit = request.stdout_limit;
    let stderr_limit = request.stderr_limit;
    let stdout_thread = thread::spawn(move || read_bounded(stdout, stdout_limit));
    let stderr_thread = thread::spawn(move || read_bounded(stderr, stderr_limit));

    let deadline = started + Duration::from_millis(request.deadline_ms);
    let mut timed_out = false;
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| ProcessError::Io(error.to_string()))?
        {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                timed_out = true;
                child
                    .kill()
                    .map_err(|error| ProcessError::Io(error.to_string()))?;
                break child
                    .wait()
                    .map_err(|error| ProcessError::Io(error.to_string()))?;
            }
            None => thread::sleep(Duration::from_millis(5)),
        }
    };
    let (stdout, stdout_truncated) = stdout_thread
        .join()
        .map_err(|_| ProcessError::Io("stdout reader panicked".to_owned()))?;
    let (stderr, stderr_truncated) = stderr_thread
        .join()
        .map_err(|_| ProcessError::Io("stderr reader panicked".to_owned()))?;
    Ok(ProcessResult {
        status: ExitStatus {
            code: status.code(),
            success: status.success(),
        },
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        timed_out,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

fn read_bounded<R: Read>(mut reader: R, limit: usize) -> (Vec<u8>, bool) {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if bytes.len() < limit {
                    let remaining = limit - bytes.len();
                    bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                    if count > remaining {
                        truncated = true;
                    }
                } else if count > 0 {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (bytes, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_argv_captures_output_and_status() {
        let request = ProcessRequest::new("printf", vec!["hello".to_owned()]);
        let result = run_bounded(&request).unwrap();
        assert!(result.status.success);
        assert_eq!(result.stdout, b"hello");
        assert!(!result.timed_out);
    }

    #[test]
    fn capture_is_bounded() {
        let mut request = ProcessRequest::new("printf", vec!["0123456789".to_owned()]);
        request.stdout_limit = 4;
        let result = run_bounded(&request).unwrap();
        assert_eq!(result.stdout, b"0123");
        assert!(result.stdout_truncated);
    }

    #[test]
    fn deadline_terminates_explicit_program() {
        let mut request = ProcessRequest::new("sleep", vec!["1".to_owned()]);
        request.deadline_ms = 10;
        let result = run_bounded(&request).unwrap();
        assert!(result.timed_out);
        assert!(!result.status.success);
    }
}
