//! Structured task scopes over verified MNCS artifacts (index PRESS-001/002).
//!
//! A [`TaskScope`] is the runtime realization of the
//! `mncs.std.scope.v1`/`mncs.std.channel.v1` source contracts: it runs
//! many entrypoint calls of ONE verified artifact with structured
//! lifetime (open/run/close), deterministic id-ordered merge, failure
//! aggregation that stays distinguishable from cancellation, and
//! cooperative cancellation observed between tasks.
//!
//! Separation of semantics from mechanism (RFC 0010): nothing here is
//! source-visible parallelism. Host threads are a realization detail —
//! results merge by work-item index, never by completion order, so thread
//! scheduling can never leak into semantic identity. No fairness,
//! deadlock-freedom, lock-freedom, or scheduler-independence property is
//! claimed; the evidence is convergence plus join guarantees, not proof.
//!
//! Execution model, stated plainly:
//! - the artifact is verified once at scope open; every worker re-opens
//!   its session from those same verified bytes, so a tampered byte
//!   anywhere refuses the whole scope before any task runs;
//! - each worker thread owns its session exclusively (no shared executor
//!   state), and `std::thread::scope` joins every thread on all paths,
//!   so no task leaks even when a sibling fails;
//! - cancellation is cooperative between tasks: a task cancelled before
//!   it starts never executes and never publishes a value, and every
//!   such task counts as executed cleanup evidence. An in-flight bounded
//!   call runs to its step budget (the same bound the `mncs-index`
//!   subprocess driver enforces with its timeout); there is no unbounded
//!   blocking anywhere in the executor, so no wakeup mechanism is owed.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use mncs_model::ExecutionValue;
use serde::{Deserialize, Serialize};

use crate::{Artifact, CallOptions, CallOutput, EmbedError, Session};

/// One unit of scoped work: a named entrypoint plus canonical ABI
/// arguments and per-task authority (step budget plus explicit grants).
/// Authority is per task, never ambient to the scope.
#[derive(Debug, Clone)]
pub struct WorkItem {
    pub module: String,
    pub function: String,
    pub args: Vec<ExecutionValue>,
    pub options: CallOptions,
}

impl WorkItem {
    pub fn new(
        module: &str,
        function: &str,
        args: Vec<ExecutionValue>,
        options: CallOptions,
    ) -> Self {
        Self {
            module: module.to_owned(),
            function: function.to_owned(),
            args,
            options,
        }
    }

    pub fn args_json(
        module: &str,
        function: &str,
        args_json: &str,
        options: CallOptions,
    ) -> Result<Self, EmbedError> {
        let args: Vec<ExecutionValue> = serde_json::from_str(args_json).map_err(|error| {
            EmbedError::new("bad_arguments", format!("argument JSON rejected: {error}"))
        })?;
        Ok(Self::new(module, function, args, options))
    }
}

/// One scoped result, always stored at its work-item index. `status` is
/// one of `returned`, `invalid_request`, `runtime_failure`,
/// `unsupported`, `budget_exhausted`, or `cancelled`. Cancelled tasks
/// carry no values and no failure reason: cancellation is distinct from
/// failure by construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedOutput {
    pub index: usize,
    pub status: String,
    pub returned: Vec<ExecutionValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub artifact_sha256: String,
    pub cancelled: bool,
}

/// Deterministic merge of one scope run. `results` is in work-item index
/// order. `first_failure` is the lowest index whose status is neither
/// `returned` nor `cancelled` — an index, never a completion timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeRun {
    pub results: Vec<ScopedOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_failure: Option<usize>,
    pub executed: usize,
    pub cancelled_count: usize,
    pub cleanups: usize,
    pub artifact_sha256: String,
}

/// Bounded parallelism for one verified artifact. See the module docs
/// for the execution model.
pub struct TaskScope {
    artifact_bytes: Vec<u8>,
    artifact_sha256: String,
    artifact_identity: String,
    backend: String,
    capacity: usize,
    closed: AtomicBool,
    cancelled: AtomicBool,
    cleanups: AtomicUsize,
}

impl TaskScope {
    /// Verify the artifact once and retain its bytes for worker sessions.
    /// Worker count per concurrent run is bounded by `capacity`
    /// (clamped to 1..=64).
    pub fn open(artifact: &Artifact, capacity: usize) -> Result<Self, EmbedError> {
        Ok(Self {
            artifact_bytes: artifact.to_json_bytes(),
            artifact_sha256: artifact.digest().to_owned(),
            artifact_identity: artifact.artifact_identity().to_owned(),
            backend: artifact.backend_name().to_owned(),
            capacity: capacity.clamp(1, 64),
            closed: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            cleanups: AtomicUsize::new(0),
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn artifact_sha256(&self) -> &str {
        &self.artifact_sha256
    }

    pub fn artifact_identity(&self) -> &str {
        &self.artifact_identity
    }

    pub fn backend_name(&self) -> &str {
        &self.backend
    }

    /// Observe cancellation: tasks not yet started stop being started.
    /// In-flight bounded calls run to their step budget.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Refuse all further runs. Pending work is never executed after
    /// close: children cannot escape their scope.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn cleanups(&self) -> usize {
        self.cleanups.load(Ordering::SeqCst)
    }

    fn check_open(&self) -> Result<(), EmbedError> {
        if self.is_closed() {
            return Err(EmbedError::new(
                "scope_closed",
                "task scope is closed; no further runs are accepted",
            ));
        }
        Ok(())
    }

    fn cancelled_output(&self, index: usize) -> ScopedOutput {
        self.cleanups.fetch_add(1, Ordering::SeqCst);
        ScopedOutput {
            index,
            status: "cancelled".to_owned(),
            returned: Vec::new(),
            failure_reason: None,
            artifact_sha256: self.artifact_sha256.clone(),
            cancelled: true,
        }
    }

    fn reopen_session(&self) -> Result<Session, EmbedError> {
        let artifact = Artifact::from_json(&self.artifact_bytes).map_err(|error| {
            EmbedError::new(
                "invalid_artifact",
                format!("scoped artifact bytes no longer validate: {error}"),
            )
        })?;
        Session::open(artifact)
    }

    fn call_once(session: &Session, index: usize, item: &WorkItem) -> ScopedOutput {
        let output: CallOutput = session.call(
            &item.module,
            &item.function,
            item.args.clone(),
            &item.options,
        );
        ScopedOutput {
            index,
            status: output.status,
            returned: output.returned,
            failure_reason: output.failure_reason,
            artifact_sha256: output.artifact_sha256,
            cancelled: false,
        }
    }

    fn finish(&self, mut results: Vec<ScopedOutput>) -> ScopeRun {
        results.sort_by_key(|result| result.index);
        let mut first_failure = None;
        let mut executed = 0;
        let mut cancelled_count = 0;
        for result in &results {
            if result.cancelled {
                cancelled_count += 1;
            } else {
                executed += 1;
                if result.status != "returned" && first_failure.is_none() {
                    first_failure = Some(result.index);
                }
            }
        }
        ScopeRun {
            results,
            first_failure,
            executed,
            cancelled_count,
            cleanups: self.cleanups(),
            artifact_sha256: self.artifact_sha256.clone(),
        }
    }

    /// Run every item sequentially on one session opened once for the
    /// run. Cancellation and close are observed between items.
    pub fn run_batch(&self, items: Vec<WorkItem>) -> Result<ScopeRun, EmbedError> {
        self.check_open()?;
        let session = self.reopen_session()?;
        let mut results = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            if self.is_closed() || self.cancelled.load(Ordering::SeqCst) {
                results.push(self.cancelled_output(index));
                continue;
            }
            results.push(Self::call_once(&session, index, item));
        }
        Ok(self.finish(results))
    }

    /// Run every item across at most `capacity` worker threads, each
    /// with its own session over the same verified bytes. Merge is by
    /// work-item index. `std::thread::scope` joins every thread on all
    /// paths: no task leaks when a sibling fails or cancels.
    pub fn run_concurrent(&self, items: Vec<WorkItem>) -> Result<ScopeRun, EmbedError> {
        self.check_open()?;
        if items.is_empty() {
            return Ok(self.finish(Vec::new()));
        }
        let workers = self.capacity.min(items.len());
        let chunk = items.len().div_ceil(workers);
        let outputs = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (chunk_id, piece) in items.chunks(chunk).enumerate() {
                let base = chunk_id * chunk;
                handles.push(scope.spawn(move || {
                    if self.is_closed() || self.cancelled.load(Ordering::SeqCst) {
                        return piece
                            .iter()
                            .enumerate()
                            .map(|(offset, _)| self.cancelled_output(base + offset))
                            .collect::<Vec<_>>();
                    }
                    let session = match self.reopen_session() {
                        Ok(session) => session,
                        Err(_) => {
                            return piece
                                .iter()
                                .enumerate()
                                .map(|(offset, _)| ScopedOutput {
                                    index: base + offset,
                                    status: "unsupported".to_owned(),
                                    returned: Vec::new(),
                                    failure_reason: Some(
                                        "scoped session refused the verified bytes".to_owned(),
                                    ),
                                    artifact_sha256: self.artifact_sha256.clone(),
                                    cancelled: false,
                                })
                                .collect::<Vec<_>>();
                        }
                    };
                    piece
                        .iter()
                        .enumerate()
                        .map(|(offset, item)| {
                            let index = base + offset;
                            if self.is_closed() || self.cancelled.load(Ordering::SeqCst) {
                                self.cancelled_output(index)
                            } else {
                                Self::call_once(&session, index, item)
                            }
                        })
                        .collect::<Vec<_>>()
                }));
            }
            let mut outputs = Vec::with_capacity(items.len());
            for handle in handles {
                outputs.extend(handle.join().expect("scoped worker thread joined"));
            }
            outputs
        });
        Ok(self.finish(outputs))
    }
}
