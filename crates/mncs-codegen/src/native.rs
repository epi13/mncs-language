//! External toolchain probing and host execution for textual realizations.
//!
//! Clang, llc, and gcc are outside the MNCS semantic trust boundary. Their
//! presence, version, flags, and exit status are observations.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use mncs_model::{BackendValueContract, ExecutionStatus, ExecutionValue, SemanticId};

use crate::support::{argument_argv, backend_output_value_from_i128};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainIdentity {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
}

impl ToolchainIdentity {
    pub fn semantic_id(&self) -> SemanticId {
        SemanticId(format!(
            "mncs:toolchain:{}:{}",
            self.name,
            crate::support::sha256_hex(
                format!("{}|{}", self.path.display(), self.version).as_bytes()
            )
        ))
    }

    pub fn summary(&self) -> String {
        format!(
            "{} {} ({})",
            self.name,
            self.version.lines().next().unwrap_or("unknown").trim(),
            self.path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeError {
    ToolchainUnavailable(String),
    UnsupportedTarget(String),
    CompileFailed(String),
    ExecutionFailed(String),
    InvalidOutput(String),
}

impl NativeError {
    pub fn status(&self) -> ExecutionStatus {
        match self {
            Self::ExecutionFailed(_) => ExecutionStatus::RuntimeFailure,
            Self::InvalidOutput(_) => ExecutionStatus::InvalidRequest,
            Self::ToolchainUnavailable(_) | Self::UnsupportedTarget(_) | Self::CompileFailed(_) => {
                ExecutionStatus::Unsupported
            }
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::ToolchainUnavailable(reason) => format!("unavailable toolchain: {reason}"),
            Self::UnsupportedTarget(reason) => format!("unsupported target: {reason}"),
            Self::CompileFailed(reason) => format!("compiler failure: {reason}"),
            Self::ExecutionFailed(reason) => format!("execution failure: {reason}"),
            Self::InvalidOutput(reason) => format!("invalid native observation: {reason}"),
        }
    }
}

pub fn probe_tool(name: &str) -> Option<ToolchainIdentity> {
    let output = Command::new(name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        return None;
    }
    Some(ToolchainIdentity {
        name: name.to_owned(),
        path: PathBuf::from(name),
        version,
    })
}

pub fn probe_clang() -> Option<ToolchainIdentity> {
    probe_tool("clang")
}

pub fn probe_llc() -> Option<ToolchainIdentity> {
    probe_tool("llc")
}

pub fn probe_gcc() -> Option<ToolchainIdentity> {
    probe_tool("gcc").or_else(|| probe_tool("cc"))
}

pub fn host_triple() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "unknown-unknown-unknown"
    }
}

pub fn host_matches_triple(triple: &str) -> bool {
    let host = host_triple();
    triple == host
        || (host.starts_with("x86_64") && triple.starts_with("x86_64"))
        || (host.starts_with("aarch64") && triple.starts_with("aarch64"))
}

#[derive(Debug, Deserialize)]
struct NativeObservation {
    status: String,
    #[serde(default)]
    value: Option<i128>,
    #[serde(default)]
    arena_hex: Option<String>,
}

/// As `compile_and_run`, with an optional canonical call file whose path is
/// exported to the child through `MNCS_CALL_FILE` (composite marshaling).
/// Preserves the full observation so composite results can decode.
pub fn compile_and_run_with_call_file_full(
    sources: &[(&str, &str)],
    compiler: &ToolchainIdentity,
    flags: &[&str],
    args: &[String],
    call_file: Option<&Path>,
) -> Result<(crate::support::NativeRunView, String), NativeError> {
    let digest = crate::support::sha256_hex(
        sources
            .iter()
            .flat_map(|(name, body)| format!("{name}\n{body}").into_bytes())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let dir = std::env::temp_dir().join(format!("mncs-native-{}", &digest[..16]));
    fs::create_dir_all(&dir).map_err(|error| {
        NativeError::CompileFailed(format!("unable to create work directory: {error}"))
    })?;
    let mut paths = Vec::new();
    for (name, body) in sources {
        let path = dir.join(name);
        fs::write(&path, body.as_bytes()).map_err(|error| {
            NativeError::CompileFailed(format!("unable to write {name}: {error}"))
        })?;
        paths.push(path);
    }
    let exe = dir.join("mncs-run");
    let mut command = Command::new(&compiler.path);
    command.current_dir(&dir);
    command.args(flags);
    for path in &paths {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("ll") => {
                command
                    .arg("-x")
                    .arg("ir")
                    .arg(path.file_name().unwrap_or(path.as_os_str()));
            }
            Some("c") => {
                command
                    .arg("-x")
                    .arg("c")
                    .arg(path.file_name().unwrap_or(path.as_os_str()));
            }
            _ => {
                command.arg(path.file_name().unwrap_or(path.as_os_str()));
            }
        }
    }
    command.arg("-o").arg(&exe);
    let compiled = command.output().map_err(|error| {
        NativeError::ToolchainUnavailable(format!(
            "{} could not be executed: {error}",
            compiler.name
        ))
    })?;
    if !compiled.status.success() {
        return Err(NativeError::CompileFailed(format!(
            "{} failed: {}",
            compiler.name,
            String::from_utf8_lossy(&compiled.stderr)
        )));
    }
    let run = run_executable_full(&exe, args, call_file)?;
    Ok((
        crate::support::NativeRunView {
            status: run.status,
            value: run.value,
            arena_hex: run.arena_hex,
        },
        compiler.summary(),
    ))
}

pub(crate) struct NativeRun {
    pub status: ExecutionStatus,
    pub value: i128,
    pub arena_hex: Option<String>,
}

fn run_executable_full(
    exe: &Path,
    args: &[String],
    call_file: Option<&Path>,
) -> Result<NativeRun, NativeError> {
    let mut run_command = Command::new(exe);
    if let Some(path) = call_file {
        run_command.env("MNCS_CALL_FILE", path);
    }
    let output = run_command
        .args(args)
        .output()
        .map_err(|error| NativeError::ExecutionFailed(error.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.trim().starts_with('{'))
        .ok_or_else(|| {
            NativeError::InvalidOutput(format!(
                "native program produced no JSON observation (stderr: {})",
                String::from_utf8_lossy(&output.stderr)
            ))
        })?;
    let observation: NativeObservation = serde_json::from_str(line.trim()).map_err(|error| {
        NativeError::InvalidOutput(format!("native JSON observation is invalid: {error}"))
    })?;
    match observation.status.as_str() {
        "returned" => Ok(NativeRun {
            status: ExecutionStatus::Returned,
            value: observation.value.ok_or_else(|| {
                NativeError::InvalidOutput("returned observation lacks a value".to_owned())
            })?,
            arena_hex: observation.arena_hex,
        }),
        "runtime_failure" => Ok(NativeRun {
            status: ExecutionStatus::RuntimeFailure,
            value: 0,
            arena_hex: None,
        }),
        other => Err(NativeError::InvalidOutput(format!(
            "unknown native status {other:?}"
        ))),
    }
}

#[allow(dead_code)]
pub fn decode_native_value(
    contract: Option<&BackendValueContract>,
    status: ExecutionStatus,
    value: i128,
) -> Result<Vec<ExecutionValue>, NativeError> {
    if status != ExecutionStatus::Returned {
        return Ok(Vec::new());
    }
    let Some(contract) = contract else {
        return Ok(vec![ExecutionValue::Integer {
            value,
            ty: mncs_model::IntegerType {
                bits: 64,
                signed: true,
            },
        }]);
    };
    backend_output_value_from_i128(contract, value).map(|value| vec![value])
}

pub fn argv_from_request(request: &mncs_model::ExecutionRequest) -> Result<Vec<String>, String> {
    request.arguments.iter().map(argument_argv).collect()
}

/// Link a native object file against a generated C driver and execute it.
/// Returns the driver's scalar JSON observation. A process killed by a
/// hardware trap (for example SIGFPE from an unguarded division) classifies
/// as a runtime failure, matching the reference executors' rejection of the
/// same input.
#[allow(dead_code)]
pub fn compile_object_and_run(
    object_name: &str,
    object_bytes: &[u8],
    driver_name: &str,
    driver_source: &str,
    linker: &ToolchainIdentity,
    args: &[String],
) -> Result<(ExecutionStatus, i128), NativeError> {
    compile_object_and_run_full(
        object_name,
        object_bytes,
        driver_name,
        driver_source,
        linker,
        args,
        None,
    )
    .map(|run| (run.status, run.value))
}

/// As `compile_object_and_run`, exporting `MNCS_CALL_FILE` to the child and
/// preserving the full observation for composite decoding.
pub fn compile_object_and_run_full(
    object_name: &str,
    object_bytes: &[u8],
    driver_name: &str,
    driver_source: &str,
    linker: &ToolchainIdentity,
    args: &[String],
    call_file: Option<&Path>,
) -> Result<crate::support::NativeRunView, NativeError> {
    let digest = crate::support::sha256_hex(object_bytes);
    let dir = std::env::temp_dir().join(format!("mncs-aot-{}", &digest[..16]));
    fs::create_dir_all(&dir).map_err(|error| {
        NativeError::CompileFailed(format!("unable to create work directory: {error}"))
    })?;
    let object_path = dir.join(object_name);
    let driver_path = dir.join(driver_name);
    fs::write(&object_path, object_bytes)
        .map_err(|error| NativeError::CompileFailed(format!("unable to write object: {error}")))?;
    fs::write(&driver_path, driver_source)
        .map_err(|error| NativeError::CompileFailed(format!("unable to write driver: {error}")))?;
    let exe = dir.join("mncs-run");
    let compiled = Command::new(&linker.path)
        .current_dir(&dir)
        .arg("-O0")
        .arg("-o")
        .arg(&exe)
        .arg(driver_path.file_name().unwrap_or(driver_path.as_os_str()))
        .arg(object_path.file_name().unwrap_or(object_path.as_os_str()))
        .output()
        .map_err(|error| {
            NativeError::ToolchainUnavailable(format!(
                "{} could not be executed: {error}",
                linker.name
            ))
        })?;
    if !compiled.status.success() {
        return Err(NativeError::CompileFailed(format!(
            "{} link failed: {}",
            linker.name,
            String::from_utf8_lossy(&compiled.stderr)
        )));
    }
    match run_executable_full(&exe, args, call_file) {
        Ok(run) => Ok(crate::support::NativeRunView {
            status: run.status,
            value: run.value,
            arena_hex: run.arena_hex,
        }),
        Err(NativeError::InvalidOutput(_)) => Ok(crate::support::NativeRunView {
            status: ExecutionStatus::RuntimeFailure,
            value: 0,
            arena_hex: None,
        }),
        Err(other) => Err(other),
    }
}
