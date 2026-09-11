//! Minimal host-callable in-process execution surface for verified MNCS
//! artifacts (P-010). This removes the CLI subprocess from the authority
//! path: a host loads one verified/frozen artifact, retains a session that
//! reuses artifact-level preparation across calls, selects a named
//! entrypoint per call, supplies typed values through the canonical
//! ABI/value encoding, and receives a structured result carrying the exact
//! artifact identity and digest actually executed.
//!
//! Authority discipline, unchanged from the CLI:
//! - the artifact digest/identity in every result corresponds to the bytes
//!   executed: no recompilation, no substitution;
//! - an artifact whose identity does not validate is refused at open;
//! - calls without grants run under the default effect policy; host
//!   effects realize only with explicit per-call grants, never ambiently;
//! - unsupported backends report `Unsupported`, never a faked value.
//!
//! The C ABI (`mncs_session_*`) is the stable boundary for non-Rust hosts
//! (including Python via ctypes): JSON in, JSON out, no Rust layout.

use std::collections::BTreeSet;

use mncs_codegen::OwnedExecutionSession;
use mncs_model::{
    BackendArtifact, EffectExecutionPolicy, ExecutionPolicy, ExecutionRequest, ExecutionStatus,
    ExecutionTarget, ExecutionValue, HostGrant, EXECUTION_REQUEST_SCHEMA_VERSION,
    HOST_GRANT_MAX_BYTES,
};
use serde::{Deserialize, Serialize};

pub mod scope;
pub use scope::{ScopeRun, ScopedOutput, TaskScope, WorkItem};

/// Machine-readable embed failure. `code` is stable for host matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedError {
    pub code: String,
    pub message: String,
}

impl EmbedError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// One verified artifact: parsed bytes plus a validated identity. The
/// digest below is the `bytes_sha256` of the artifact actually executed.
pub struct Artifact {
    inner: BackendArtifact,
}

impl Artifact {
    /// Load frozen artifact bytes (the `backend-artifact.json` document).
    /// Refuses artifacts whose identity does not validate: a tampered or
    /// truncated artifact never becomes a session.
    pub fn from_json(bytes: &[u8]) -> Result<Self, EmbedError> {
        let artifact: BackendArtifact = serde_json::from_slice(bytes).map_err(|error| {
            EmbedError::new(
                "invalid_artifact",
                format!("artifact JSON rejected: {error}"),
            )
        })?;
        if !artifact.identity_is_valid() {
            return Err(EmbedError::new(
                "invalid_identity",
                "backend artifact identity does not validate; refusing",
            ));
        }
        Ok(Self { inner: artifact })
    }

    /// Compile self-contained source text to an artifact in-process (no
    /// subprocess). `backend` names a registered adapter
    /// (`mncs-research-bytecode`, `mncs-portable-wasm-mvp`, `mncs-c11`,
    /// `mncs-llvm-ir`, `mncs-cranelift`). Sources with `use` imports are
    /// refused here: embedding executes frozen artifacts, and library
    /// resolution belongs to the build step that froze them.
    pub fn from_source(source: &str, backend: &str) -> Result<Self, EmbedError> {
        use mncs_compiler::ReferenceCompiler;
        use mncs_model::ArtifactRepresentation;
        use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

        let compiler = ReferenceCompiler::default();
        let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "embed-source", source);
        let front_end = compiler.front_end(envelope);
        if !front_end.is_valid() {
            return Err(EmbedError::new(
                "compile_failed",
                "source front end is invalid; no artifact was produced",
            ));
        }
        let Some(program) = front_end.program else {
            return Err(EmbedError::new(
                "compile_failed",
                "source front end produced no program",
            ));
        };
        let emit: BTreeSet<ArtifactRepresentation> = [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
            ArtifactRepresentation::TargetLoweringPlan,
            ArtifactRepresentation::BackendArtifact,
        ]
        .into_iter()
        .collect();
        let request = compiler
            .request_for_program_with_backend(&program, emit, backend)
            .map_err(|diagnostic| {
                EmbedError::new("unsupported_backend", format!("{diagnostic:?}"))
            })?;
        let compilation = compiler.compile(request, &program);
        // The CLI `experiment run` path executes artifacts that completed
        // with unresolved obligations under their conservative fallbacks,
        // so embedding matches that contract: either completed status is
        // executable, and the frozen artifact carries its evidence bundle
        // (including the retained obligations) under its identity.
        if !matches!(
            compilation.status,
            mncs_model::CompilationStatus::Completed
                | mncs_model::CompilationStatus::CompletedWithUnresolvedObligations
        ) {
            return Err(EmbedError::new(
                "compile_failed",
                "compilation did not complete; no artifact was produced",
            ));
        }
        let Some(artifact) = compilation.emissions.backend else {
            return Err(EmbedError::new(
                "compile_failed",
                "compilation emitted no backend artifact",
            ));
        };
        Self::from_json(&serde_json::to_vec(&artifact).map_err(|error| {
            EmbedError::new(
                "invalid_artifact",
                format!("artifact re-serialization failed: {error}"),
            )
        })?)
    }

    /// Re-serialize the verified artifact (e.g. to ship frozen bytes to
    /// another process that opens them with [`Session`] or the C ABI).
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.inner).expect("verified artifact serializes")
    }

    pub fn digest(&self) -> &str {
        &self.inner.bytes_sha256
    }

    pub fn artifact_identity(&self) -> &str {
        &self.inner.identity.0
    }

    pub fn backend_name(&self) -> &str {
        &self.inner.backend.name
    }
}

/// Explicit per-call host authority. Read grants carry bounded bytes;
/// write grants carry a destination path; time/crypto grants carry only
/// the capability name. Nothing is ambient: a call without grants runs
/// under the default policy and every host call in it fails closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub capability: String,
    #[serde(default)]
    pub locator: String,
    #[serde(default)]
    pub bytes: Vec<u8>,
}

impl Grant {
    pub fn read_bytes(capability: &str, locator: &str, bytes: Vec<u8>) -> Result<Self, EmbedError> {
        if bytes.len() > HOST_GRANT_MAX_BYTES {
            return Err(EmbedError::new(
                "grant_too_large",
                format!("read grant exceeds the {HOST_GRANT_MAX_BYTES}-byte bound; refusing"),
            ));
        }
        Ok(Self {
            capability: capability.to_owned(),
            locator: locator.to_owned(),
            bytes,
        })
    }

    pub fn write_path(capability: &str, path: &str) -> Self {
        Self {
            capability: capability.to_owned(),
            locator: path.to_owned(),
            bytes: Vec::new(),
        }
    }

    /// Filesystem-root grant: `path` names the granted root the `fs_*`
    /// intrinsics may enumerate, chunk-read (Profile 0.12), and mutate
    /// through the `fs_write` family (Profile 0.16). Canonicalization
    /// and containment happen at realization; a bogus root fails the
    /// call closed, never the scope.
    pub fn fs_root(capability: &str, path: &str) -> Self {
        Self {
            capability: capability.to_owned(),
            locator: path.to_owned(),
            bytes: Vec::new(),
        }
    }

    pub fn time(capability: &str) -> Self {
        Self {
            capability: capability.to_owned(),
            locator: "host-clock".to_owned(),
            bytes: Vec::new(),
        }
    }

    pub fn crypto(capability: &str) -> Self {
        Self {
            capability: capability.to_owned(),
            locator: "crypto-verify".to_owned(),
            bytes: Vec::new(),
        }
    }

    fn into_host_grant(self) -> HostGrant {
        HostGrant {
            capability: self.capability,
            locator: self.locator,
            bytes: self.bytes,
        }
    }
}

/// Per-call options: step budget plus explicit grants. No grants means no
/// realized host effects on that call.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub step_budget: u64,
    pub grants: Vec<Grant>,
}

impl CallOptions {
    pub fn budgeted(step_budget: u64) -> Self {
        Self {
            step_budget,
            grants: Vec::new(),
        }
    }
}

/// Structured per-call result. `artifact_identity`/`artifact_sha256`
/// identify the exact bytes executed on this call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallOutput {
    pub status: String,
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<mncs_model::ExecutionEffectEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub artifact_identity: String,
    pub artifact_sha256: String,
    pub backend: String,
    pub reused_session: bool,
}

/// A retained execution session for one verified artifact. Artifact-level
/// preparation (validation, identity, block indexes, decoded modules)
/// happens once at open; every call still runs the request-specific
/// checks. Results are observationally identical to one-shot execution.
pub struct Session {
    inner: OwnedExecutionSession,
}

impl Session {
    pub fn open(artifact: Artifact) -> Result<Self, EmbedError> {
        let inner = OwnedExecutionSession::new(artifact.inner)
            .map_err(|message| EmbedError::new("invalid_artifact", message))?;
        Ok(Self { inner })
    }

    pub fn reused(&self) -> bool {
        self.inner.reused()
    }

    pub fn digest(&self) -> &str {
        &self.inner.artifact().bytes_sha256
    }

    pub fn artifact_identity(&self) -> &str {
        &self.inner.artifact().identity.0
    }

    pub fn backend_name(&self) -> &str {
        &self.inner.artifact().backend.name
    }

    /// Execute one named entrypoint with canonical ABI values. A
    /// zero `step_budget` in `options` selects the default bound.
    pub fn call(
        &self,
        module: &str,
        function: &str,
        arguments: Vec<ExecutionValue>,
        options: &CallOptions,
    ) -> CallOutput {
        let step_budget = if options.step_budget == 0 {
            8_192
        } else {
            options.step_budget
        };
        let grants: Vec<HostGrant> = options
            .grants
            .iter()
            .cloned()
            .map(Grant::into_host_grant)
            .collect();
        let mut request = ExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: module.to_owned(),
                function: function.to_owned(),
            },
            arguments,
            step_budget,
            policy: ExecutionPolicy::default(),
            host_grants: Vec::new(),
            call_depth_budget: None,
        };
        if !grants.is_empty() {
            request = request.with_host_grants(&grants);
            request.policy.effects = EffectExecutionPolicy::Realize;
        }
        let observation = self.inner.execute(&request);
        let status = match observation.status {
            ExecutionStatus::Returned => "returned",
            ExecutionStatus::InvalidRequest => "invalid_request",
            ExecutionStatus::RuntimeFailure => "runtime_failure",
            ExecutionStatus::Unsupported => "unsupported",
            ExecutionStatus::BudgetExhausted => "budget_exhausted",
        }
        .to_owned();
        CallOutput {
            status,
            returned: observation.returned,
            steps: observation.steps,
            effects: observation.effects,
            failure_reason: observation.failure.map(|failure| failure.reason),
            artifact_identity: self.artifact_identity().to_owned(),
            artifact_sha256: self.digest().to_owned(),
            backend: self.backend_name().to_owned(),
            reused_session: self.reused(),
        }
    }

    /// JSON convenience: `args_json` is a JSON array of canonical
    /// [`ExecutionValue`] documents; returns the [`CallOutput`] document.
    pub fn call_json(
        &self,
        module: &str,
        function: &str,
        args_json: &str,
        options: &CallOptions,
    ) -> Result<CallOutput, EmbedError> {
        let arguments: Vec<ExecutionValue> = serde_json::from_str(args_json).map_err(|error| {
            EmbedError::new("bad_arguments", format!("argument JSON rejected: {error}"))
        })?;
        Ok(self.call(module, function, arguments, options))
    }
}

// ---- Stable C ABI (no Rust layout crosses) -------------------------------
//
// `mncs_session_call` and `mncs_session_info` return an owned response
// handle; `mncs_response_text` borrows NUL-terminated UTF-8 JSON out of
// it, valid until `mncs_response_free`. Every other handle is opaque.
// Single-threaded host discipline per session.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uchar};
use std::ptr;
use std::slice;

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("ok").expect("ok"));
}

fn stash_error(message: String) {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() =
            CString::new(message).unwrap_or_else(|_| CString::new("error").expect("error"));
    });
}

/// English detail for the last failing call on this thread.
///
/// # Safety
///
/// Always safe: takes no pointer arguments and returns a thread-local string.
#[no_mangle]
pub unsafe extern "C" fn mncs_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

fn read_c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(str::to_owned) }
}

/// Owned JSON response document. Opaque to hosts.
pub struct CallResponse {
    text: CString,
}

impl CallResponse {
    fn of(value: &impl Serialize) -> Result<*mut Self, String> {
        let text = serde_json::to_string(value)
            .map_err(|error| format!("response JSON failed: {error}"))?;
        let text = CString::new(text).map_err(|error| format!("response text failed: {error}"))?;
        Ok(Box::into_raw(Box::new(Self { text })))
    }
}

/// Open a session from frozen artifact bytes. Returns NULL on failure;
/// consult `mncs_last_error`.
///
/// # Safety
///
/// `bytes` must point to `len` readable bytes for the call; NULL fails closed with no read.
#[no_mangle]
pub unsafe extern "C" fn mncs_session_open(bytes: *const c_uchar, len: usize) -> *mut Session {
    if bytes.is_null() {
        stash_error("null artifact bytes".to_owned());
        return ptr::null_mut();
    }
    let raw = unsafe { slice::from_raw_parts(bytes, len) };
    let opened = Artifact::from_json(raw)
        .map_err(|error| error.to_string())
        .and_then(|artifact| Session::open(artifact).map_err(|error| error.to_string()));
    match opened {
        Ok(session) => Box::into_raw(Box::new(session)),
        Err(message) => {
            stash_error(message);
            ptr::null_mut()
        }
    }
}

/// Close a session opened by `mncs_session_open`. NULL is a no-op.
///
/// # Safety
///
/// `handle` must be NULL or a live handle from `mncs_session_open`, closed at most once.
#[no_mangle]
pub unsafe extern "C" fn mncs_session_close(handle: *mut Session) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

/// Session identity: artifact identity/digest, backend, and whether
/// artifact-level preparation is reused across calls. NULL on failure.
///
/// # Safety
///
/// `handle` must be NULL or a live session handle.
#[no_mangle]
pub unsafe extern "C" fn mncs_session_info(handle: *const Session) -> *mut CallResponse {
    if handle.is_null() {
        stash_error("null session handle".to_owned());
        return ptr::null_mut();
    }
    let session = unsafe { &*handle };
    let info = serde_json::json!({
        "artifact_identity": session.artifact_identity(),
        "artifact_sha256": session.digest(),
        "backend": session.backend_name(),
        "reused_session": session.reused(),
    });
    match CallResponse::of(&info) {
        Ok(response) => response,
        Err(message) => {
            stash_error(message);
            ptr::null_mut()
        }
    }
}

/// Execute one entrypoint; `args_json` is a JSON array of canonical ABI
/// values, `grants_json` a JSON array of [`Grant`] (or NULL/empty for
/// none). Returns the [`CallOutput`] JSON document, or NULL on failure.
///
/// # Safety
///
/// `handle` must be NULL or a live session handle; string pointers must be NULL or valid NUL-terminated UTF-8.
#[no_mangle]
pub unsafe extern "C" fn mncs_session_call(
    handle: *const Session,
    module: *const c_char,
    function: *const c_char,
    args_json: *const c_char,
    grants_json: *const c_char,
    step_budget: u64,
) -> *mut CallResponse {
    if handle.is_null() {
        stash_error("null session handle".to_owned());
        return ptr::null_mut();
    }
    let session = unsafe { &*handle };
    let (Some(module), Some(function)) = (read_c_str(module), read_c_str(function)) else {
        stash_error("null module or function name".to_owned());
        return ptr::null_mut();
    };
    let args_text = read_c_str(args_json).unwrap_or_else(|| "[]".to_owned());
    let grants_text = read_c_str(grants_json).unwrap_or_default();
    let grants: Vec<Grant> = if grants_text.trim().is_empty() {
        Vec::new()
    } else {
        match serde_json::from_str(&grants_text) {
            Ok(grants) => grants,
            Err(error) => {
                stash_error(format!("grant JSON rejected: {error}"));
                return ptr::null_mut();
            }
        }
    };
    let options = CallOptions {
        step_budget,
        grants,
    };
    match session.call_json(&module, &function, &args_text, &options) {
        Ok(output) => match CallResponse::of(&output) {
            Ok(response) => response,
            Err(message) => {
                stash_error(message);
                ptr::null_mut()
            }
        },
        Err(error) => {
            stash_error(error.to_string());
            ptr::null_mut()
        }
    }
}

/// Execute many entrypoints sequentially on one session with one boundary
/// crossing; `requests_json` is a JSON array of
/// `{module, function, args, grants?, step_budget?}` objects. Returns the
/// JSON array of [`CallOutput`] documents in request order, or NULL on
/// failure. This is the stable-boundary batch API for hosts that issue
/// hundreds of kernel calls per build (index PRESS-010): per-call
/// subprocess cost is gone, and even the per-call ABI crossing amortizes
/// to one call here.
///
/// # Safety
///
/// `handle` must be NULL or a live session handle; `requests_json` must
/// be NULL or valid NUL-terminated UTF-8.
#[no_mangle]
pub unsafe extern "C" fn mncs_session_call_batch(
    handle: *const Session,
    requests_json: *const c_char,
) -> *mut CallResponse {
    if handle.is_null() {
        stash_error("null session handle".to_owned());
        return ptr::null_mut();
    }
    let session = unsafe { &*handle };
    let text = read_c_str(requests_json).unwrap_or_else(|| "[]".to_owned());
    #[derive(serde::Deserialize)]
    struct BatchRequest {
        module: String,
        function: String,
        #[serde(default)]
        args: Vec<mncs_model::ExecutionValue>,
        #[serde(default)]
        grants: Vec<Grant>,
        #[serde(default)]
        step_budget: u64,
    }
    let requests: Vec<BatchRequest> = match serde_json::from_str(&text) {
        Ok(requests) => requests,
        Err(error) => {
            stash_error(format!("batch request JSON rejected: {error}"));
            return ptr::null_mut();
        }
    };
    let outputs: Vec<CallOutput> = requests
        .into_iter()
        .map(|request| {
            let options = CallOptions {
                step_budget: request.step_budget,
                grants: request.grants,
            };
            session.call(&request.module, &request.function, request.args, &options)
        })
        .collect();
    match CallResponse::of(&outputs) {
        Ok(response) => response,
        Err(message) => {
            stash_error(message);
            ptr::null_mut()
        }
    }
}

/// Borrow NUL-terminated UTF-8 JSON out of a response. Valid until
/// `mncs_response_free`. NULL in gives NULL out.
///
/// # Safety
///
/// `response` must be NULL or a live response handle.
#[no_mangle]
pub unsafe extern "C" fn mncs_response_text(response: *const CallResponse) -> *const c_char {
    if response.is_null() {
        return ptr::null();
    }
    unsafe { (*response).text.as_ptr() }
}

/// Free a response from `mncs_session_call`/`mncs_session_call_batch`/
/// `mncs_session_info`. NULL is a no-op.
///
/// # Safety
///
/// `response` must be NULL or a live response handle, freed at most once.
#[no_mangle]
pub unsafe extern "C" fn mncs_response_free(response: *mut CallResponse) {
    if !response.is_null() {
        unsafe {
            drop(Box::from_raw(response));
        }
    }
}
