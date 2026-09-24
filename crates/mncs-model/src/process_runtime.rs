//! Owned external-work lifecycle used by MNCS host effects.
//!
//! `process_run` remains the bounded synchronous convenience operation in
//! [`crate::process`].  This module supplies the long-lived capability needed
//! when another semantic event must address work that is already running.
//! Handles are provider-issued 256-bit bearer capabilities scoped to one
//! runtime instance; they are never PIDs and cannot address work in another
//! runtime.  The Linux realization uses a transient systemd service whose
//! cgroup owns the complete process tree.  Platforms without an equivalent
//! provider return `Unsupported` instead of approximating tree ownership.

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::process::{MAX_DEADLINE_MS, MAX_ENVIRONMENT_BYTES, ProcessRequest};

const MAX_ACTIVE_EXECUTIONS: usize = 16;
const MAX_RETAINED_EXECUTIONS: usize = 128;
const CONTROL_COMMAND_TIMEOUT: Duration = Duration::from_millis(300);
const START_GRACE: Duration = Duration::from_millis(600);
const TERMINATE_GRACE: Duration = Duration::from_millis(300);
const REAP_POLL: Duration = Duration::from_millis(5);
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Platform-neutral resource limits for one owned process tree.
///
/// `None` means that the caller did not request that particular limit.  The
/// values describe semantics; the provider decides how to realize them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessResourceEnvelope {
    pub memory_high_bytes: Option<u64>,
    pub memory_max_bytes: Option<u64>,
    pub swap_max_bytes: Option<u64>,
    pub process_max: Option<u64>,
}

/// Request for a process that must outlive the source call that starts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedProcessRequest {
    pub process: ProcessRequest,
    pub resources: ProcessResourceEnvelope,
}

impl OwnedProcessRequest {
    pub fn validate(&self) -> Result<(), ProcessRuntimeError> {
        self.process
            .validate()
            .map_err(|error| ProcessRuntimeError::Invalid(error.to_string()))?;
        let resources = &self.resources;
        if let Some(high) = resources.memory_high_bytes {
            if high == 0
                || resources
                    .memory_max_bytes
                    .is_some_and(|maximum| high > maximum)
            {
                return Err(ProcessRuntimeError::Invalid(
                    "memory_high_bytes must be positive and no greater than memory_max_bytes"
                        .to_owned(),
                ));
            }
        }
        if resources.memory_max_bytes == Some(0)
            || resources.process_max == Some(0)
            || resources.memory_max_bytes.is_some_and(|maximum| {
                resources
                    .memory_high_bytes
                    .is_some_and(|high| high > maximum)
            })
        {
            return Err(ProcessRuntimeError::Invalid(
                "resource envelope contains an inconsistent or zero hard limit".to_owned(),
            ));
        }
        if self.process.environment.iter().any(|(key, value)| {
            key.as_bytes().contains(&0)
                || value.as_bytes().contains(&0)
                || key.is_empty()
                || key.contains('=')
        }) || self
            .process
            .environment
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>()
            > MAX_ENVIRONMENT_BYTES
        {
            return Err(ProcessRuntimeError::Invalid(
                "environment exceeds its bounded process contract".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Observable lifecycle state. `Cancelled` is only returned after the
/// provider has established an empty owned cgroup and reaped its launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLifecycleStatus {
    Running,
    Exited,
    Cancelled,
    TimedOut,
    ResourceExhausted,
    OutputExhausted,
    Unsupported,
    Unknown,
}

/// Raw, typed process and cleanup observations.  The booleans intentionally
/// remain independent so MNCS policy can interpret resource outcomes without
/// inheriting a provider's classification precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessObservation {
    pub status: ProcessLifecycleStatus,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub output_exhausted: bool,
    pub deadline_exceeded: bool,
    pub cancellation_requested: bool,
    pub cancellation_complete: bool,
    pub memory_high_events: Option<u64>,
    pub memory_max_events: Option<u64>,
    pub oom_events: Option<u64>,
    pub oom_kill_events: Option<u64>,
    pub process_limit_events: Option<u64>,
    pub memory_peak_bytes: Option<u64>,
    pub swap_peak_bytes: Option<u64>,
    pub process_peak: Option<u64>,
    pub containment_supported: bool,
    pub tree_empty: Option<bool>,
    pub launcher_reaped: bool,
    pub cleanup_complete: Option<bool>,
    pub observation_complete: bool,
    pub duration_ms: u64,
}

impl ProcessObservation {
    /// An incomplete provider observation for an operation that could not
    /// establish process state. The caller preserves this as UNKNOWN or as
    /// explicit unsupported containment; it is never a semantic failure.
    pub fn incomplete(status: ProcessLifecycleStatus) -> Self {
        Self {
            status,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            output_exhausted: false,
            deadline_exceeded: false,
            cancellation_requested: false,
            cancellation_complete: false,
            memory_high_events: None,
            memory_max_events: None,
            oom_events: None,
            oom_kill_events: None,
            process_limit_events: None,
            memory_peak_bytes: None,
            swap_peak_bytes: None,
            process_peak: None,
            containment_supported: false,
            tree_empty: None,
            launcher_reaped: false,
            cleanup_complete: None,
            observation_complete: false,
            duration_ms: 0,
        }
    }
}

/// Errors distinguish an invalid request from absent runtime support and
/// incomplete lifecycle observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessRuntimeError {
    Invalid(String),
    Unsupported(String),
    Unknown(String),
    UnknownHandle,
    Limit(String),
}

impl std::fmt::Display for ProcessRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid process request: {message}"),
            Self::Unsupported(message) => write!(f, "process lifecycle unsupported: {message}"),
            Self::Unknown(message) => write!(f, "process lifecycle observation unknown: {message}"),
            Self::UnknownHandle => f.write_str("process handle is not owned by this runtime"),
            Self::Limit(message) => write!(f, "process lifecycle limit: {message}"),
        }
    }
}

impl std::error::Error for ProcessRuntimeError {}

/// Provider-issued, runtime-scoped capability for one running external
/// execution. Its token is private and its debug representation is redacted.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OwnedProcessHandle {
    token: [u8; 32],
}

impl std::fmt::Debug for OwnedProcessHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnedProcessHandle([redacted])")
    }
}

impl OwnedProcessHandle {
    /// The opaque token is carried by a standard MNCS handle value.  It is a
    /// bearer capability, not a PID; the receiving runtime still checks that
    /// it issued and currently owns the token.
    pub fn opaque_token(&self) -> [u8; 32] {
        self.token
    }
}

#[derive(Debug)]
struct CapturedStream {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl CapturedStream {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(64 * 1024)),
            limit,
            exceeded: false,
        }
    }

    fn append(&mut self, chunk: &[u8]) {
        let remaining = self.limit.saturating_sub(self.bytes.len());
        self.bytes
            .extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        self.exceeded |= chunk.len() > remaining;
    }
}

struct ManagedProcess {
    child: Mutex<Child>,
    program: String,
    unit: String,
    cgroup: Mutex<Option<PathBuf>>,
    stdout: Arc<Mutex<CapturedStream>>,
    stderr: Arc<Mutex<CapturedStream>>,
    stdout_reader: Mutex<Option<JoinHandle<()>>>,
    stderr_reader: Mutex<Option<JoinHandle<()>>>,
    stdin_writer: Mutex<Option<JoinHandle<()>>>,
    reader_stop: Arc<AtomicBool>,
    started: Instant,
    deadline: Instant,
    cancellation_requested: Mutex<bool>,
    cancellation_started: Mutex<Option<Instant>>,
    timed_out: Mutex<bool>,
    output_exhausted: Mutex<bool>,
    finalized: Mutex<Option<ProcessObservation>>,
    provider_ready: AtomicBool,
    active_slots: Arc<AtomicUsize>,
    slot_released: AtomicBool,
}

/// The generic process capability owned by one retained MNCS runtime.
/// Dropping it requests cancellation and performs bounded best-effort reap
/// for every still-live execution; failed cleanup remains UNKNOWN.
pub struct ProcessRuntime {
    executions: Mutex<HashMap<[u8; 32], Arc<ManagedProcess>>>,
    active_slots: Arc<AtomicUsize>,
    registry_slots: AtomicUsize,
}

impl Default for ProcessRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRuntime {
    pub fn new() -> Self {
        Self {
            executions: Mutex::new(HashMap::new()),
            active_slots: Arc::new(AtomicUsize::new(0)),
            registry_slots: AtomicUsize::new(0),
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_slots.load(Ordering::Acquire)
    }

    /// Start work and return its provider-issued capability without waiting
    /// for completion. A missing Linux containment provider is fail-closed.
    pub fn start(
        &self,
        request: OwnedProcessRequest,
    ) -> Result<(OwnedProcessHandle, ProcessObservation), ProcessRuntimeError> {
        request.validate()?;
        self.reserve_slot()?;
        let mut token = [0u8; 32];
        if let Err(error) = getrandom::getrandom(&mut token) {
            self.active_slots.fetch_sub(1, Ordering::AcqRel);
            return Err(ProcessRuntimeError::Unknown(format!(
                "secure handle creation failed: {error}"
            )));
        }
        if let Err(error) = self.reserve_registry_slot() {
            self.active_slots.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        let unit = format!("mncs-owned-{}.service", hex(&token[..16]));
        let managed = match start_systemd_process(&unit, &request, Arc::clone(&self.active_slots)) {
            Ok(managed) => Arc::new(managed),
            Err(error) => {
                self.active_slots.fetch_sub(1, Ordering::AcqRel);
                self.registry_slots.fetch_sub(1, Ordering::AcqRel);
                return Err(error);
            }
        };
        let handle = OwnedProcessHandle { token };
        lock(&self.executions).insert(token, Arc::clone(&managed));
        let observation = observe_managed(&managed)
            .unwrap_or_else(|_| ProcessObservation::incomplete(ProcessLifecycleStatus::Unknown));
        Ok((handle, observation))
    }

    /// Reconstruct a source-carried handle only when this provider issued it.
    pub fn handle_from_token(
        &self,
        token: [u8; 32],
    ) -> Result<OwnedProcessHandle, ProcessRuntimeError> {
        if lock(&self.executions).contains_key(&token) {
            Ok(OwnedProcessHandle { token })
        } else {
            Err(ProcessRuntimeError::UnknownHandle)
        }
    }

    pub fn program_for_handle(
        &self,
        handle: &OwnedProcessHandle,
    ) -> Result<String, ProcessRuntimeError> {
        self.lookup(handle).map(|process| process.program.clone())
    }

    pub fn observe(
        &self,
        handle: &OwnedProcessHandle,
    ) -> Result<ProcessObservation, ProcessRuntimeError> {
        let managed = self.lookup(handle)?;
        if Instant::now() >= managed.deadline && lock(&managed.finalized).is_none() {
            *lock(&managed.timed_out) = true;
            let _ = request_managed_cancel(&managed);
        }
        if *lock(&managed.cancellation_requested)
            && lock(&managed.cancellation_started)
                .is_some_and(|started| started.elapsed() >= TERMINATE_GRACE)
            && lock(&managed.finalized).is_none()
        {
            let _ = systemctl(&["kill", "--kill-whom=all", "--signal=SIGKILL", &managed.unit]);
        }
        observe_managed(&managed)
    }

    /// Send a cancellation request. This returns before claiming cancellation
    /// complete; callers must observe/reap until cleanup is established.
    pub fn request_cancel(
        &self,
        handle: &OwnedProcessHandle,
    ) -> Result<ProcessObservation, ProcessRuntimeError> {
        let managed = self.lookup(handle)?;
        let _ = request_managed_cancel(&managed);
        observe_managed(&managed)
    }

    /// Wait for termination and reaping up to the supplied bound. If cleanup
    /// cannot be established, the returned observation stays UNKNOWN.
    pub fn reap(
        &self,
        handle: &OwnedProcessHandle,
        wait_bound: Duration,
    ) -> Result<ProcessObservation, ProcessRuntimeError> {
        let deadline = Instant::now() + wait_bound.min(Duration::from_secs(30));
        loop {
            let observation = self.observe(handle)?;
            if observation.cleanup_complete == Some(true)
                || observation.status == ProcessLifecycleStatus::Unsupported
                || Instant::now() >= deadline
            {
                return Ok(observation);
            }
            thread::sleep(REAP_POLL);
        }
    }

    /// Wait until the owned operation reaches a terminal state, honoring its
    /// declared deadline and returning the same raw observation as `reap`.
    pub fn wait(
        &self,
        handle: &OwnedProcessHandle,
    ) -> Result<ProcessObservation, ProcessRuntimeError> {
        let managed = self.lookup(handle)?;
        let deadline = Instant::now()
            + managed
                .deadline
                .saturating_duration_since(Instant::now())
                .saturating_add(Duration::from_secs(2));
        loop {
            let observation = self.observe(handle)?;
            if observation.cleanup_complete == Some(true)
                || observation.status == ProcessLifecycleStatus::Unsupported
                || Instant::now() >= deadline
            {
                return Ok(observation);
            }
            thread::sleep(REAP_POLL);
        }
    }

    fn lookup(
        &self,
        handle: &OwnedProcessHandle,
    ) -> Result<Arc<ManagedProcess>, ProcessRuntimeError> {
        lock(&self.executions)
            .get(&handle.token)
            .cloned()
            .ok_or(ProcessRuntimeError::UnknownHandle)
    }

    fn reserve_slot(&self) -> Result<(), ProcessRuntimeError> {
        self.active_slots
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_ACTIVE_EXECUTIONS).then_some(active + 1)
            })
            .map(|_| ())
            .map_err(|_| {
                ProcessRuntimeError::Limit(format!(
                    "at most {MAX_ACTIVE_EXECUTIONS} active owned executions are allowed"
                ))
            })
    }

    fn reserve_registry_slot(&self) -> Result<(), ProcessRuntimeError> {
        // Give terminal handles a bounded retained lifetime. First observe
        // entries without holding the registry lock (provider queries can
        // block briefly), then evict only entries whose terminal cleanup was
        // established. Unknown cleanup state is never silently discarded.
        let snapshot = lock(&self.executions).values().cloned().collect::<Vec<_>>();
        for process in snapshot {
            let _ = observe_managed(&process);
        }
        let mut executions = lock(&self.executions);
        while self.registry_slots.load(Ordering::Acquire) >= MAX_RETAINED_EXECUTIONS {
            let completed = executions
                .iter()
                .find(|(_, process)| lock(&process.finalized).is_some())
                .map(|(token, _)| *token);
            if let Some(completed) = completed {
                executions.remove(&completed);
                self.registry_slots.fetch_sub(1, Ordering::AcqRel);
            } else {
                return Err(ProcessRuntimeError::Limit(
                    "owned execution registry is full because cleanup is unproven".to_owned(),
                ));
            }
        }
        self.registry_slots.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

impl Drop for ProcessRuntime {
    fn drop(&mut self) {
        let executions: Vec<_> = lock(&self.executions)
            .iter()
            .map(|(token, process)| (*token, Arc::clone(process)))
            .collect();
        for (_token, process) in executions {
            let _ = request_managed_cancel(&process);
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if lock(&process.cancellation_started)
                    .is_some_and(|started| started.elapsed() >= TERMINATE_GRACE)
                {
                    let _ =
                        systemctl(&["kill", "--kill-whom=all", "--signal=SIGKILL", &process.unit]);
                }
                if observe_managed(&process)
                    .ok()
                    .is_some_and(|observation| observation.cleanup_complete == Some(true))
                {
                    break;
                }
                thread::sleep(REAP_POLL);
            }
            process.reader_stop.store(true, Ordering::Release);
            join_reader(&process.stdout_reader);
            join_reader(&process.stderr_reader);
            join_reader(&process.stdin_writer);
        }
    }
}

fn start_systemd_process(
    unit: &str,
    request: &OwnedProcessRequest,
    active_slots: Arc<AtomicUsize>,
) -> Result<ManagedProcess, ProcessRuntimeError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (unit, request, active_slots);
        return Err(ProcessRuntimeError::Unsupported(
            "no process-tree and aggregate-resource provider is registered for this platform"
                .to_owned(),
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let systemd_run = find_program("systemd-run").ok_or_else(|| {
            ProcessRuntimeError::Unsupported("systemd-run is unavailable".to_owned())
        })?;
        let env = find_program("env").ok_or_else(|| {
            ProcessRuntimeError::Unsupported(
                "the explicit-environment launcher is unavailable".to_owned(),
            )
        })?;
        let mut command = Command::new(systemd_run);
        command
            .args([
                "--user",
                "--wait",
                "--pipe",
                "--quiet",
                &format!("--unit={unit}"),
                "--slice=mncs-processes.slice",
                &format!(
                    "--working-directory={}",
                    request
                        .process
                        .current_dir
                        .as_deref()
                        .unwrap_or_else(|| Path::new("."))
                        .display()
                ),
                &format!("--property=KillMode=control-group"),
                "--property=TimeoutStopSec=1s",
                "--property=MemoryAccounting=yes",
                "--property=TasksAccounting=yes",
            ])
            .arg(format!(
                "--property=RuntimeMaxSec={}ms",
                request.process.deadline_ms
            ));
        if let Some(value) = request.resources.memory_high_bytes {
            command.arg(format!("--property=MemoryHigh={value}"));
        }
        if let Some(value) = request.resources.memory_max_bytes {
            command.arg(format!("--property=MemoryMax={value}"));
        }
        if let Some(value) = request.resources.swap_max_bytes {
            command.arg(format!("--property=MemorySwapMax={value}"));
        }
        if let Some(value) = request.resources.process_max {
            command.arg(format!("--property=TasksMax={value}"));
        }
        command.arg("--").arg(env).arg("-i");
        for (key, value) in &request.process.environment {
            command.arg(format!("{key}={value}"));
        }
        command
            .arg(&request.process.program)
            .args(&request.process.argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            command.env("XDG_RUNTIME_DIR", runtime_dir);
        }
        for name in ["DBUS_SESSION_BUS_ADDRESS", "SYSTEMD_BUS_ADDRESS"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command.spawn().map_err(|error| {
            ProcessRuntimeError::Unsupported(format!(
                "unable to start systemd process provider: {error}"
            ))
        })?;
        let stdin_writer = if let Some(mut stdin) = child.stdin.take() {
            let input = request.process.stdin.clone();
            Some(thread::spawn(move || {
                let _ = stdin.write_all(&input);
            }))
        } else {
            None
        };
        let stdout = Arc::new(Mutex::new(CapturedStream::new(
            request.process.stdout_limit,
        )));
        let stderr = Arc::new(Mutex::new(CapturedStream::new(
            request.process.stderr_limit,
        )));
        let reader_stop = Arc::new(AtomicBool::new(false));
        let stdout_reader = child
            .stdout
            .take()
            .map(|stream| spawn_reader(stream, Arc::clone(&stdout), Arc::clone(&reader_stop)));
        let stderr_reader = child
            .stderr
            .take()
            .map(|stream| spawn_reader(stream, Arc::clone(&stderr), Arc::clone(&reader_stop)));
        let started = Instant::now();
        let managed = ManagedProcess {
            child: Mutex::new(child),
            program: request.process.program.clone(),
            unit: unit.to_owned(),
            cgroup: Mutex::new(None),
            stdout,
            stderr,
            stdout_reader: Mutex::new(stdout_reader),
            stderr_reader: Mutex::new(stderr_reader),
            stdin_writer: Mutex::new(stdin_writer),
            reader_stop,
            started,
            deadline: started
                + Duration::from_millis(request.process.deadline_ms.min(MAX_DEADLINE_MS)),
            cancellation_requested: Mutex::new(false),
            cancellation_started: Mutex::new(None),
            timed_out: Mutex::new(false),
            output_exhausted: Mutex::new(false),
            finalized: Mutex::new(None),
            provider_ready: AtomicBool::new(false),
            active_slots,
            slot_released: AtomicBool::new(false),
        };
        let ready_deadline = started + START_GRACE;
        loop {
            let (properties, complete) = systemd_properties(unit);
            if let Some(group) = properties.get("ControlGroup") {
                if let Some(path) = cgroup_path(group) {
                    *lock(&managed.cgroup) = Some(path);
                }
            }
            if properties
                .get("LoadState")
                .is_some_and(|state| state == "loaded")
                && properties.get("Id").is_some_and(|id| id == unit)
            {
                managed.provider_ready.store(true, Ordering::Release);
                return Ok(managed);
            }
            let exited = lock(&managed.child).try_wait().ok().flatten();
            if exited.is_some() || Instant::now() >= ready_deadline {
                let _ = complete;
                return Ok(managed);
            }
            thread::sleep(REAP_POLL);
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_reader<R: Read + std::os::fd::AsRawFd + Send + 'static>(
    mut stream: R,
    capture: Arc<Mutex<CapturedStream>>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let descriptor = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let mut descriptor = descriptor;
        let mut buffer = [0u8; 8192];
        loop {
            let ready = unsafe { libc::poll(&mut descriptor, 1, 5) };
            if ready == 0 {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                continue;
            }
            if ready < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => lock(&capture).append(&buffer[..count]),
            }
        }
    })
}

#[cfg(not(target_os = "linux"))]
fn spawn_reader<R: Read + Send + 'static>(
    mut stream: R,
    capture: Arc<Mutex<CapturedStream>>,
    _stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => lock(&capture).append(&buffer[..count]),
            }
        }
    })
}

fn request_managed_cancel(process: &ManagedProcess) -> Result<(), ProcessRuntimeError> {
    if lock(&process.finalized).is_some() || *lock(&process.cancellation_requested) {
        return Ok(());
    }
    *lock(&process.cancellation_requested) = true;
    *lock(&process.cancellation_started) = Some(Instant::now());
    let result = systemctl(&["kill", "--kill-whom=all", "--signal=SIGTERM", &process.unit]);
    if result.is_none_or(|(status, _)| !status.success()) {
        return Err(ProcessRuntimeError::Unknown(
            "systemd did not acknowledge process-tree cancellation".to_owned(),
        ));
    }
    Ok(())
}

fn observe_managed(process: &ManagedProcess) -> Result<ProcessObservation, ProcessRuntimeError> {
    if let Some(observation) = lock(&process.finalized).clone() {
        return Ok(observation);
    }
    let (properties, control_complete) = systemd_properties(&process.unit);
    if let Some(group) = properties.get("ControlGroup") {
        if let Some(path) = cgroup_path(group) {
            *lock(&process.cgroup) = Some(path);
        }
    }
    if properties
        .get("LoadState")
        .is_some_and(|state| state == "loaded")
        && properties.get("Id").is_some_and(|id| id == &process.unit)
    {
        process.provider_ready.store(true, Ordering::Release);
    }
    let cgroup = lock(&process.cgroup).clone();
    let mut raw = cgroup.as_deref().map(read_cgroup_observations);
    // systemd retains the unit's peak value after it removes the transient
    // cgroup. Preserve that provider observation rather than returning an
    // empty metric simply because a fast terminal process outlived its cgroup
    // directory.
    if let Some(raw) = raw.as_mut() {
        if raw.memory_peak_bytes.is_none() {
            raw.memory_peak_bytes = properties
                .get("MemoryPeak")
                .and_then(|value| value.parse::<u64>().ok());
        }
        if properties
            .get("Result")
            .is_some_and(|result| matches!(result.as_str(), "oom" | "oom-kill"))
        {
            // systemd's OOM result proves a resource event, but a zero cgroup
            // counter sampled after unit teardown conflicts with that result.
            // Preserve the status and mark those counters unavailable rather
            // than reporting a false zero or fabricating an exact count.
            let mut counter_conflict = false;
            for counter in [
                &mut raw.memory_max_events,
                &mut raw.oom_events,
                &mut raw.oom_kill_events,
            ] {
                if *counter == Some(0) {
                    *counter = None;
                    counter_conflict = true;
                }
            }
            raw.complete &= !counter_conflict;
        }
    }
    let exit_status = lock(&process.child).try_wait().map_err(|error| {
        ProcessRuntimeError::Unknown(format!("process wait observation failed: {error}"))
    })?;
    let stdout = lock(&process.stdout);
    let stderr = lock(&process.stderr);
    let output_exhausted = stdout.exceeded || stderr.exceeded;
    drop(stdout);
    drop(stderr);
    if output_exhausted && !*lock(&process.output_exhausted) {
        *lock(&process.output_exhausted) = true;
        let _ = request_managed_cancel(process);
    }
    let reaped = exit_status.is_some();
    let unit_inactive = properties
        .get("ActiveState")
        .is_some_and(|state| state == "inactive")
        || properties
            .get("SubState")
            .is_some_and(|state| matches!(state.as_str(), "dead" | "failed"));
    // systemd removes a transient unit's cgroup after KillMode=control-group
    // has completed. An inactive unit with an empty ControlGroup is the
    // provider's equivalent proof of cgroup.events populated=0.
    let tree_empty = raw.as_ref().and_then(|value| value.tree_empty).or_else(|| {
        (control_complete
            && unit_inactive
            && properties.get("ControlGroup").is_some_and(String::is_empty))
        .then_some(true)
    });
    let cleanup_complete = match (reaped, unit_inactive, tree_empty) {
        (true, true, Some(true)) => Some(true),
        (true, true, Some(false)) | (false, _, _) => Some(false),
        _ => None,
    };
    let terminal = reaped && cleanup_complete == Some(true);
    let timed_out =
        *lock(&process.timed_out) || properties.get("Result").is_some_and(|v| v == "timeout");
    let resource_exhausted = raw.as_ref().is_some_and(|value| {
        value.memory_max_events.is_some_and(|n| n > 0)
            || value.oom_events.is_some_and(|n| n > 0)
            || value.oom_kill_events.is_some_and(|n| n > 0)
            || value.process_limit_events.is_some_and(|n| n > 0)
    }) || properties
        .get("Result")
        .is_some_and(|result| matches!(result.as_str(), "oom" | "oom-kill"));
    let provider_ready = process.provider_ready.load(Ordering::Acquire);
    let status = if cleanup_complete.is_none() && reaped {
        ProcessLifecycleStatus::Unknown
    } else if terminal && !provider_ready {
        ProcessLifecycleStatus::Unknown
    } else if terminal && *lock(&process.output_exhausted) {
        ProcessLifecycleStatus::OutputExhausted
    } else if terminal && timed_out {
        ProcessLifecycleStatus::TimedOut
    } else if terminal && *lock(&process.cancellation_requested) {
        ProcessLifecycleStatus::Cancelled
    } else if terminal && resource_exhausted {
        ProcessLifecycleStatus::ResourceExhausted
    } else if terminal {
        ProcessLifecycleStatus::Exited
    } else {
        ProcessLifecycleStatus::Running
    };
    if terminal {
        if let Some(status) = exit_status {
            let terminal_status = if !provider_ready {
                ProcessLifecycleStatus::Unknown
            } else if output_exhausted {
                ProcessLifecycleStatus::OutputExhausted
            } else if timed_out {
                ProcessLifecycleStatus::TimedOut
            } else if resource_exhausted {
                ProcessLifecycleStatus::ResourceExhausted
            } else if *lock(&process.cancellation_requested) {
                ProcessLifecycleStatus::Cancelled
            } else {
                ProcessLifecycleStatus::Exited
            };
            process.reader_stop.store(true, Ordering::Release);
            join_reader(&process.stdout_reader);
            join_reader(&process.stderr_reader);
            join_reader(&process.stdin_writer);
            let observation = make_observation(
                process,
                status,
                raw,
                control_complete,
                cleanup_complete,
                tree_empty,
                terminal_status,
            );
            // Retire the transient provider unit after recording its final
            // properties and cgroup counters. The provider runtime retains
            // the typed terminal observation; systemd no longer needs to
            // retain a failed-unit tombstone for this completed handle.
            let _ = systemctl(&["reset-failed", &process.unit]);
            *lock(&process.finalized) = Some(observation.clone());
            if !process.slot_released.swap(true, Ordering::AcqRel) {
                process.active_slots.fetch_sub(1, Ordering::AcqRel);
            }
            return Ok(observation);
        }
    }
    let _ = status;
    Ok(observation_from_live(
        process,
        properties,
        raw,
        control_complete,
        cleanup_complete,
        status,
        output_exhausted,
    ))
}

fn make_observation(
    process: &ManagedProcess,
    exit: ExitStatus,
    raw: Option<CgroupObservations>,
    control_complete: bool,
    cleanup_complete: Option<bool>,
    tree_empty: Option<bool>,
    status: ProcessLifecycleStatus,
) -> ProcessObservation {
    let stdout = lock(&process.stdout);
    let stderr = lock(&process.stderr);
    let output_exhausted = *lock(&process.output_exhausted);
    let deadline_exceeded = *lock(&process.timed_out);
    let cancellation_requested = *lock(&process.cancellation_requested);
    let containment_supported = process.cgroup.lock().is_ok_and(|path| path.is_some());
    ProcessObservation {
        status,
        exit_code: exit.code(),
        stdout: stdout.bytes.clone(),
        stderr: stderr.bytes.clone(),
        stdout_truncated: stdout.exceeded,
        stderr_truncated: stderr.exceeded,
        output_exhausted,
        deadline_exceeded,
        cancellation_requested,
        cancellation_complete: cancellation_requested && cleanup_complete == Some(true),
        memory_high_events: raw.as_ref().and_then(|value| value.memory_high_events),
        memory_max_events: raw.as_ref().and_then(|value| value.memory_max_events),
        oom_events: raw.as_ref().and_then(|value| value.oom_events),
        oom_kill_events: raw.as_ref().and_then(|value| value.oom_kill_events),
        process_limit_events: raw.as_ref().and_then(|value| value.process_limit_events),
        memory_peak_bytes: raw.as_ref().and_then(|value| value.memory_peak_bytes),
        swap_peak_bytes: raw.as_ref().and_then(|value| value.swap_peak_bytes),
        process_peak: raw.as_ref().and_then(|value| value.process_peak),
        containment_supported,
        tree_empty,
        launcher_reaped: true,
        cleanup_complete,
        observation_complete: control_complete && raw.as_ref().is_some_and(|value| value.complete),
        duration_ms: process
            .started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64,
    }
}

fn observation_from_live(
    process: &ManagedProcess,
    _properties: HashMap<String, String>,
    raw: Option<CgroupObservations>,
    control_complete: bool,
    cleanup_complete: Option<bool>,
    status: ProcessLifecycleStatus,
    output_exhausted: bool,
) -> ProcessObservation {
    let stdout = lock(&process.stdout);
    let stderr = lock(&process.stderr);
    ProcessObservation {
        status,
        exit_code: None,
        stdout: stdout.bytes.clone(),
        stderr: stderr.bytes.clone(),
        stdout_truncated: stdout.exceeded,
        stderr_truncated: stderr.exceeded,
        output_exhausted,
        deadline_exceeded: *lock(&process.timed_out),
        cancellation_requested: *lock(&process.cancellation_requested),
        cancellation_complete: false,
        memory_high_events: raw.as_ref().and_then(|value| value.memory_high_events),
        memory_max_events: raw.as_ref().and_then(|value| value.memory_max_events),
        oom_events: raw.as_ref().and_then(|value| value.oom_events),
        oom_kill_events: raw.as_ref().and_then(|value| value.oom_kill_events),
        process_limit_events: raw.as_ref().and_then(|value| value.process_limit_events),
        memory_peak_bytes: raw.as_ref().and_then(|value| value.memory_peak_bytes),
        swap_peak_bytes: raw.as_ref().and_then(|value| value.swap_peak_bytes),
        process_peak: raw.as_ref().and_then(|value| value.process_peak),
        containment_supported: process.cgroup.lock().is_ok_and(|path| path.is_some()),
        tree_empty: raw.as_ref().and_then(|value| value.tree_empty),
        launcher_reaped: false,
        cleanup_complete,
        observation_complete: control_complete && raw.as_ref().is_some_and(|value| value.complete),
        duration_ms: process
            .started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64,
    }
}

#[derive(Debug, Default)]
struct CgroupObservations {
    complete: bool,
    memory_high_events: Option<u64>,
    memory_max_events: Option<u64>,
    oom_events: Option<u64>,
    oom_kill_events: Option<u64>,
    process_limit_events: Option<u64>,
    memory_peak_bytes: Option<u64>,
    swap_peak_bytes: Option<u64>,
    process_peak: Option<u64>,
    tree_empty: Option<bool>,
}

fn read_cgroup_observations(path: &Path) -> CgroupObservations {
    let memory_events = read_key_values(&path.join("memory.events"));
    let pids_events = read_key_values(&path.join("pids.events"));
    let cgroup_events = read_key_values(&path.join("cgroup.events"));
    let mut observations = CgroupObservations {
        complete: false,
        memory_high_events: memory_events.get("high").copied(),
        memory_max_events: memory_events.get("max").copied(),
        oom_events: memory_events.get("oom").copied(),
        oom_kill_events: memory_events.get("oom_kill").copied(),
        process_limit_events: pids_events.get("max").copied(),
        memory_peak_bytes: read_number(&path.join("memory.peak")),
        swap_peak_bytes: read_number(&path.join("memory.swap.peak")),
        process_peak: read_number(&path.join("pids.peak")),
        tree_empty: cgroup_events.get("populated").map(|value| *value == 0),
    };
    observations.complete = observations.memory_high_events.is_some()
        && observations.memory_max_events.is_some()
        && observations.oom_events.is_some()
        && observations.oom_kill_events.is_some()
        && observations.process_limit_events.is_some()
        && observations.memory_peak_bytes.is_some()
        && observations.swap_peak_bytes.is_some()
        && observations.process_peak.is_some()
        && observations.tree_empty.is_some();
    observations
}

fn read_number(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_key_values(path: &Path) -> HashMap<String, u64> {
    std::fs::read_to_string(path)
        .ok()
        .into_iter()
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| {
                    let (key, value) = line.split_once(' ')?;
                    Some((key.to_owned(), value.parse().ok()?))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn systemd_properties(unit: &str) -> (HashMap<String, String>, bool) {
    let Some((status, stdout)) = systemctl(&[
        "show",
        unit,
        "--property=Id",
        "--property=LoadState",
        "--property=ControlGroup",
        "--property=ActiveState",
        "--property=SubState",
        "--property=Result",
        "--property=ExecMainStatus",
        "--property=MemoryPeak",
        "--property=TasksCurrent",
        "--property=CPUUsageNSec",
    ]) else {
        return (HashMap::new(), false);
    };
    let properties = String::from_utf8_lossy(&stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    (properties, status.success())
}

fn systemctl(arguments: &[&str]) -> Option<(ExitStatus, Vec<u8>)> {
    let systemctl = find_program("systemctl")?;
    let mut command = Command::new(systemctl);
    command
        .arg("--user")
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        command.env("XDG_RUNTIME_DIR", runtime_dir);
    }
    for name in ["DBUS_SESSION_BUS_ADDRESS", "SYSTEMD_BUS_ADDRESS"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        if child.try_wait().ok()?.is_some() {
            let output = child.wait_with_output().ok()?;
            return Some((output.status, output.stdout));
        }
        if started.elapsed() >= CONTROL_COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(REAP_POLL);
    }
}

fn cgroup_path(group: &str) -> Option<PathBuf> {
    if !group.starts_with('/') || group.split('/').any(|part| part == "..") {
        return None;
    }
    let path = Path::new(CGROUP_ROOT).join(group.trim_start_matches('/'));
    path.starts_with(CGROUP_ROOT).then_some(path)
}

fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn join_reader(handle: &Mutex<Option<JoinHandle<()>>>) {
    if let Some(join) = lock(handle).take() {
        let _ = join.join();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(program: &str, argv: &[&str], deadline_ms: u64) -> OwnedProcessRequest {
        let mut process =
            ProcessRequest::new(program, argv.iter().map(|arg| (*arg).to_owned()).collect());
        process.deadline_ms = deadline_ms;
        process.stdout_limit = 4096;
        process.stderr_limit = 4096;
        OwnedProcessRequest {
            process,
            resources: ProcessResourceEnvelope {
                memory_high_bytes: Some(128 * 1024 * 1024),
                memory_max_bytes: Some(256 * 1024 * 1024),
                swap_max_bytes: Some(0),
                process_max: Some(32),
            },
        }
    }

    #[test]
    fn lifecycle_handle_is_provider_issued_and_cancel_requires_reap() {
        let runtime = ProcessRuntime::new();
        let (handle, observation) = match runtime.start(request("/usr/bin/sleep", &["10"], 20_000))
        {
            Ok(value) => value,
            // Runtime support is an explicit platform capability. Unsupported
            // hosts stay UNKNOWN instead of falling back to PID-only cleanup.
            Err(ProcessRuntimeError::Unsupported(_)) => return,
            Err(error) => panic!("process lifecycle start failed: {error}"),
        };
        assert_eq!(observation.status, ProcessLifecycleStatus::Running);
        assert!(observation.containment_supported);
        assert!(runtime.handle_from_token([0; 32]).is_err());
        let requested = runtime.request_cancel(&handle).unwrap();
        assert!(requested.cancellation_requested);
        if requested.cancellation_complete {
            assert_eq!(requested.cleanup_complete, Some(true));
        }
        let completed = runtime.reap(&handle, Duration::from_secs(3)).unwrap();
        assert!(completed.cancellation_complete);
        assert_eq!(completed.cleanup_complete, Some(true));
        assert_eq!(completed.tree_empty, Some(true));
        assert!(completed.launcher_reaped);
    }

    #[test]
    fn expired_execution_is_distinct_from_external_cancellation() {
        let runtime = ProcessRuntime::new();
        let (handle, _) = match runtime.start(request("/usr/bin/sleep", &["10"], 100)) {
            Ok(value) => value,
            Err(ProcessRuntimeError::Unsupported(_)) => return,
            Err(error) => panic!("process lifecycle start failed: {error}"),
        };
        let completed = runtime.wait(&handle).unwrap();
        assert!(completed.deadline_exceeded);
        assert_eq!(completed.status, ProcessLifecycleStatus::TimedOut);
        assert_eq!(completed.cleanup_complete, Some(true));
    }

    #[test]
    fn active_execution_limit_is_enforced_and_reaped() {
        let runtime = ProcessRuntime::new();
        let mut handles = Vec::with_capacity(MAX_ACTIVE_EXECUTIONS);
        for _ in 0..MAX_ACTIVE_EXECUTIONS {
            match runtime.start(request("/usr/bin/sleep", &["30"], 60_000)) {
                Ok((handle, observation)) => {
                    assert_eq!(observation.status, ProcessLifecycleStatus::Running);
                    assert!(observation.containment_supported);
                    handles.push(handle);
                }
                Err(ProcessRuntimeError::Unsupported(_)) => return,
                Err(error) => panic!("bounded active process start failed: {error}"),
            }
        }
        assert_eq!(runtime.active_count(), MAX_ACTIVE_EXECUTIONS);
        assert!(matches!(
            runtime.start(request("/usr/bin/true", &[], 10_000)),
            Err(ProcessRuntimeError::Limit(_))
        ));
        assert_eq!(runtime.active_count(), MAX_ACTIVE_EXECUTIONS);

        for handle in handles {
            let requested = runtime.request_cancel(&handle).unwrap();
            assert!(requested.cancellation_requested);
            let completed = runtime.reap(&handle, Duration::from_secs(3)).unwrap();
            assert_eq!(completed.status, ProcessLifecycleStatus::Cancelled);
            assert_eq!(completed.cleanup_complete, Some(true));
            assert_eq!(completed.tree_empty, Some(true));
            assert!(completed.launcher_reaped);
        }
        assert_eq!(runtime.active_count(), 0);
    }

    #[test]
    fn completed_handle_retention_is_bounded_and_eviction_fails_closed() {
        let runtime = ProcessRuntime::new();
        let mut handles = Vec::with_capacity(MAX_RETAINED_EXECUTIONS + 1);
        for _ in 0..=MAX_RETAINED_EXECUTIONS {
            let (handle, _) = match runtime.start(request("/usr/bin/true", &[], 10_000)) {
                Ok(value) => value,
                Err(ProcessRuntimeError::Unsupported(_)) => return,
                Err(error) => panic!("completed process start failed: {error}"),
            };
            let completed = runtime.reap(&handle, Duration::from_secs(3)).unwrap();
            assert_eq!(completed.cleanup_complete, Some(true));
            assert_eq!(completed.tree_empty, Some(true));
            assert!(completed.launcher_reaped);
            handles.push(handle);
            assert!(runtime.registry_slots.load(Ordering::Acquire) <= MAX_RETAINED_EXECUTIONS);
            assert_eq!(runtime.active_count(), 0);
        }
        assert_eq!(
            runtime.registry_slots.load(Ordering::Acquire),
            MAX_RETAINED_EXECUTIONS
        );

        let expired = handles
            .iter()
            .find(|handle| {
                matches!(
                    runtime.observe(handle),
                    Err(ProcessRuntimeError::UnknownHandle)
                )
            })
            .expect("at least one completed handle is evicted at the retention bound");
        assert!(matches!(
            runtime.handle_from_token(expired.opaque_token()),
            Err(ProcessRuntimeError::UnknownHandle)
        ));
        for handle in &handles {
            match runtime.observe(handle) {
                Ok(observation) => {
                    assert_eq!(observation.cleanup_complete, Some(true));
                    assert!(observation.launcher_reaped);
                }
                Err(ProcessRuntimeError::UnknownHandle) if handle == expired => {}
                Err(error) => panic!("retained process observation failed: {error}"),
            }
        }
    }
}
