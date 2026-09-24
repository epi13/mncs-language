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
use std::time::Instant;

use mncs_codegen::OwnedExecutionSession;
use mncs_model::{
    BackendArtifact, EffectExecutionPolicy, ExecutionPolicy, ExecutionRequest, ExecutionStatus,
    ExecutionTarget, ExecutionTypeArgument, ExecutionValue, HostExecutionValue,
    HostGenericSeedRequest, HostGrant, SemanticId, EXECUTION_REQUEST_SCHEMA_VERSION,
    HOST_GRANT_MAX_BYTES,
};
use serde::{Deserialize, Serialize};

pub mod scope;
pub use scope::{ScopeRun, ScopedOutput, TaskScope, WorkItem};
pub mod cache;
pub use cache::{
    CompiledArtifactCache, CompiledArtifactCacheKey, COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION,
};
pub mod process;
pub mod provider;
pub mod structured;
pub use provider::{
    decode_identity, encode_identity, AdmittedProvider, ProviderDescriptor, ProviderFacts,
    ProviderRegistry, PROVIDER_DESCRIPTOR_SCHEMA_VERSION,
};

/// Machine-readable embed failure. `code` is stable for host matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedError {
    pub code: String,
    pub message: String,
}

impl EmbedError {
    /// Construct a stable host-facing error from a typed binding adapter.
    /// Generated bindings use this for local encode/decode failures while
    /// runtime failures continue to originate from the session itself.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
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
    /// truncated artifact never becomes a session. Pre-0.5 artifacts whose
    /// contract type references traveled as bare strings normalize
    /// deterministically after the identity check passes (so tampering is
    /// still refused on the original bytes) and re-validate under the
    /// upgraded schema before admission.
    pub fn from_json(bytes: &[u8]) -> Result<Self, EmbedError> {
        let mut artifact: BackendArtifact = serde_json::from_slice(bytes).map_err(|error| {
            EmbedError::new(
                "invalid_artifact",
                format!("artifact JSON rejected: {error}"),
            )
        })?;
        if let Some(identity) = artifact.ambiguous_callable_identity() {
            return Err(EmbedError::new(
                "ambiguous_callable_identity",
                format!("backend artifact declares callable identity {identity} more than once"),
            ));
        }
        if !artifact.identity_is_valid() {
            return Err(EmbedError::new(
                "invalid_identity",
                "backend artifact identity does not validate; refusing",
            ));
        }
        if artifact.schema_version == mncs_model::BACKEND_ARTIFACT_SCHEMA_VERSION_PRE_TYPED
            || artifact.schema_version == mncs_model::BACKEND_ARTIFACT_SCHEMA_VERSION_PRE_INTERFACE
        {
            artifact.normalize_legacy_contracts();
            if !artifact.identity_is_valid() {
                return Err(EmbedError::new(
                    "invalid_identity",
                    "backend artifact identity does not validate after normalization; refusing",
                ));
            }
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
        Self::from_source_with_seeds(source, backend, &[])
    }

    /// Compile self-contained source text plus host-requested generic
    /// instantiations (P1-013/P2-003): each seed compiles its
    /// specialization into the artifact, which [`Session::call`] then
    /// selects with matching `type_arguments` in [`CallOptions`].
    /// Seed-free sources behave exactly like [`Artifact::from_source`].
    pub fn from_source_with_seeds(
        source: &str,
        backend: &str,
        seeds: &[HostGenericSeedRequest],
    ) -> Result<Self, EmbedError> {
        use mncs_compiler::ReferenceCompiler;
        use mncs_model::ArtifactRepresentation;
        use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

        let compiler = ReferenceCompiler::default();
        let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "embed-source", source);
        let front_end = compiler.front_end_with_seeds(envelope, seeds);
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

/// Per-call options: step budget, explicit grants, and explicit generic
/// arguments. No grants means no realized host effects on that call;
/// empty `type_arguments` means a concrete target.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub step_budget: u64,
    pub grants: Vec<Grant>,
    /// Interface identity expected by a generated host binding.  A typed
    /// call refuses an artifact that does not carry the same language-owned
    /// callable interface, so stale generated code cannot reinterpret a
    /// changed record or enum signature.
    pub expected_interface_identity: Option<String>,
    /// Explicit generic arguments selecting a compiled specialization
    /// of a generic entrypoint (P1-013/P2-003), mirroring
    /// `ExecutionRequest.type_arguments`. Empty for concrete targets.
    /// The artifact must have compiled the instantiation — via
    /// [`Artifact::from_source_with_seeds`] or a seeded frozen
    /// artifact — or the call fails closed as `invalid_request`.
    pub type_arguments: Vec<ExecutionTypeArgument>,
}

impl CallOptions {
    pub fn budgeted(step_budget: u64) -> Self {
        Self {
            step_budget,
            grants: Vec::new(),
            expected_interface_identity: None,
            type_arguments: Vec::new(),
        }
    }
}

/// A compiler-owned callable resolved in one exact loaded artifact.
/// References are revision-bound even when the callable declaration and its
/// signature remain stable across body changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallableReference {
    pub artifact_identity: SemanticId,
    pub callable_identity: SemanticId,
    pub declaration_identity: SemanticId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_case_identity: Option<SemanticId>,
    pub signature_identity: String,
}

/// One typed invocation in a retained-session batch.  The Rust API keeps
/// values typed; the JSON/C ABI batch below is only the external
/// interoperability projection of the same operation.
#[derive(Debug, Clone)]
pub enum BatchCallTarget {
    Named { module: String, function: String },
    Callable(CallableReference),
}

#[derive(Debug, Clone)]
pub struct BatchCall {
    pub target: BatchCallTarget,
    pub arguments: Vec<ExecutionValue>,
    pub options: CallOptions,
}

impl BatchCall {
    pub fn new(
        module: impl Into<String>,
        function: impl Into<String>,
        arguments: Vec<ExecutionValue>,
        options: CallOptions,
    ) -> Self {
        Self {
            target: BatchCallTarget::Named {
                module: module.into(),
                function: function.into(),
            },
            arguments,
            options,
        }
    }

    pub fn identity(
        reference: CallableReference,
        arguments: Vec<ExecutionValue>,
        options: CallOptions,
    ) -> Self {
        Self {
            target: BatchCallTarget::Callable(reference),
            arguments,
            options,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoked_callable: Option<CallableReference>,
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
        // `Artifact::from_json`/`from_source` have already crossed the
        // artifact identity admission boundary. Avoid hashing the complete
        // artifact a second time while retaining the validating constructor
        // for callers that have not crossed that boundary.
        let inner = OwnedExecutionSession::new_admitted(artifact.inner)
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

    pub fn interface_identity(&self) -> Option<&str> {
        self.inner.artifact().interface_identity.as_deref()
    }

    /// Resolve one compiler-issued callable, declaration, or test-case
    /// identity from this loaded artifact. The returned reference records
    /// the exact artifact revision and signature that established ownership.
    pub fn callable_reference(
        &self,
        identity: &SemanticId,
    ) -> Result<CallableReference, EmbedError> {
        let artifact = self.inner.artifact();
        let matches = artifact
            .callable_bindings
            .iter()
            .filter(|binding| {
                &binding.callable_identity == identity
                    || &binding.declaration_identity == identity
                    || binding.test_case_identity.as_ref() == Some(identity)
            })
            .collect::<Vec<_>>();
        let binding = match matches.as_slice() {
            [] if artifact.callable_bindings.is_empty() => {
                return Err(EmbedError::new(
                    "missing_callable_metadata",
                    "loaded artifact predates compiler-owned callable bindings; recompile it",
                ));
            }
            [] => {
                return Err(EmbedError::new(
                    "unknown_callable_identity",
                    format!("callable identity {identity} is not present in the loaded artifact"),
                ));
            }
            [binding] => *binding,
            _ => {
                return Err(EmbedError::new(
                    "ambiguous_callable_identity",
                    format!("callable identity {identity} resolves to multiple declarations"),
                ));
            }
        };
        Ok(CallableReference {
            artifact_identity: artifact.identity.clone(),
            callable_identity: binding.callable_identity.clone(),
            declaration_identity: binding.declaration_identity.clone(),
            test_case_identity: binding.test_case_identity.clone(),
            signature_identity: binding.signature_identity.clone(),
        })
    }

    /// Execute one compiler-owned callable reference using the same runtime,
    /// typed value membrane, generic specialization table, grants, and effect
    /// policy as named invocation.
    pub fn call_identity(
        &self,
        reference: &CallableReference,
        arguments: Vec<ExecutionValue>,
        options: &CallOptions,
    ) -> Result<CallOutput, EmbedError> {
        let binding = self.verify_callable_reference(reference)?;
        self.validate_expected_interface_identity(options)?;
        self.validate_binding_arguments(binding, &arguments, &options.type_arguments)?;
        let mut output = self.call(&binding.module, &binding.function, arguments, options);
        output.invoked_callable = Some(reference.clone());
        Ok(output)
    }

    /// Name-oriented typed values dispatched by compiler-owned callable
    /// identity. Values are resolved and checked against the selected
    /// declaration before execution begins.
    pub fn call_identity_typed(
        &self,
        reference: &CallableReference,
        values: Vec<HostExecutionValue>,
        options: &CallOptions,
    ) -> Result<CallOutput, EmbedError> {
        let binding = self.verify_callable_reference(reference)?;
        self.validate_expected_interface_identity(options)?;
        self.validate_generic_arguments(binding, &options.type_arguments)?;
        let arguments = mncs_codegen::resolve_typed_arguments_for_artifact(
            self.inner.artifact(),
            &binding.module,
            &binding.function,
            &options.type_arguments,
            &values,
        )
        .map_err(|error| EmbedError::new("bad_typed_arguments", error))?;
        self.call_identity(reference, arguments, options)
    }

    fn validate_expected_interface_identity(
        &self,
        options: &CallOptions,
    ) -> Result<(), EmbedError> {
        if let Some(expected) = options.expected_interface_identity.as_deref() {
            let Some(actual) = self.interface_identity() else {
                return Err(EmbedError::new(
                    "stale_interface",
                    "artifact has no language-owned interface identity; regenerate the host binding",
                ));
            };
            if expected != actual {
                return Err(EmbedError::new(
                    "stale_interface",
                    format!(
                        "interface identity mismatch: expected {expected}, loaded {actual}; regenerate the host binding"
                    ),
                ));
            }
        }
        Ok(())
    }

    fn verify_callable_reference(
        &self,
        reference: &CallableReference,
    ) -> Result<&mncs_model::BackendCallableBinding, EmbedError> {
        let artifact = self.inner.artifact();
        if reference.artifact_identity != artifact.identity {
            return Err(EmbedError::new(
                "artifact_identity_mismatch",
                format!(
                    "callable reference belongs to artifact {}, loaded artifact is {}",
                    reference.artifact_identity, artifact.identity
                ),
            ));
        }
        let mut matches = artifact
            .callable_bindings
            .iter()
            .filter(|binding| binding.callable_identity == reference.callable_identity);
        let Some(binding) = matches.next() else {
            return Err(EmbedError::new(
                "unknown_callable_identity",
                format!(
                    "callable identity {} is not present in the loaded artifact",
                    reference.callable_identity
                ),
            ));
        };
        if matches.next().is_some() {
            return Err(EmbedError::new(
                "ambiguous_callable_identity",
                format!(
                    "callable identity {} resolves to multiple declarations",
                    reference.callable_identity
                ),
            ));
        }
        if binding.declaration_identity != reference.declaration_identity
            || binding.test_case_identity != reference.test_case_identity
        {
            return Err(EmbedError::new(
                "stale_declaration_identity",
                format!(
                    "callable {} has a different declaration or test-case identity in the loaded artifact",
                    reference.callable_identity
                ),
            ));
        }
        if binding.signature_identity != reference.signature_identity {
            return Err(EmbedError::new(
                "signature_identity_mismatch",
                format!(
                    "callable {} signature identity mismatch",
                    reference.callable_identity
                ),
            ));
        }
        Ok(binding)
    }

    fn validate_binding_arguments(
        &self,
        binding: &mncs_model::BackendCallableBinding,
        arguments: &[ExecutionValue],
        type_arguments: &[ExecutionTypeArgument],
    ) -> Result<(), EmbedError> {
        self.validate_generic_arguments(binding, type_arguments)?;
        mncs_codegen::validate_execution_arguments_for_artifact(
            self.inner.artifact(),
            &binding.module,
            &binding.function,
            type_arguments,
            arguments,
        )
        .map(|_| ())
        .map_err(|error| EmbedError::new("invalid_callable_arguments", error))
    }

    fn validate_generic_arguments(
        &self,
        binding: &mncs_model::BackendCallableBinding,
        type_arguments: &[ExecutionTypeArgument],
    ) -> Result<(), EmbedError> {
        if binding.generic_params.len() != type_arguments.len() {
            let message = if binding.generic_params.is_empty() {
                format!(
                    "callable {} takes no generic type arguments, received {}",
                    binding.callable_identity,
                    type_arguments.len()
                )
            } else if type_arguments.is_empty() {
                format!(
                    "callable {} requires {} generic type argument(s)",
                    binding.callable_identity,
                    binding.generic_params.len()
                )
            } else {
                format!(
                    "callable {} expects {} generic type argument(s), received {}",
                    binding.callable_identity,
                    binding.generic_params.len(),
                    type_arguments.len()
                )
            };
            return Err(EmbedError::new("invalid_type_arguments", message));
        }
        for (parameter, argument) in binding.generic_params.iter().zip(type_arguments) {
            let matches = matches!(
                (parameter.kind, argument),
                (
                    mncs_model::GenericParamKind::Type,
                    ExecutionTypeArgument::Type { .. }
                ) | (
                    mncs_model::GenericParamKind::Nat,
                    ExecutionTypeArgument::Nat { .. }
                )
            );
            if !matches {
                return Err(EmbedError::new(
                    "invalid_type_arguments",
                    format!(
                        "generic parameter {} expects {:?}, received a different type-argument kind",
                        parameter.name, parameter.kind
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Execute one named entrypoint with canonical ABI values. A
    /// zero `step_budget` in `options` selects the default bound.
    /// Generic entrypoints run through the explicit `type_arguments`
    /// in `options` (see [`CallOptions`]); a bare generic target fails
    /// closed as `invalid_request`, never by running the template.
    pub fn call(
        &self,
        module: &str,
        function: &str,
        arguments: Vec<ExecutionValue>,
        options: &CallOptions,
    ) -> CallOutput {
        self.call_with_runtime(module, function, arguments, options, None)
    }

    fn call_with_runtime(
        &self,
        module: &str,
        function: &str,
        arguments: Vec<ExecutionValue>,
        options: &CallOptions,
        provider_runtime: Option<&dyn mncs_model::ProviderRuntime>,
    ) -> CallOutput {
        if let Some(binding) = self
            .inner
            .artifact()
            .callable_bindings
            .iter()
            .find(|binding| binding.module == module && binding.function == function)
        {
            if let Err(error) =
                self.validate_binding_arguments(binding, &arguments, &options.type_arguments)
            {
                return self.invalid_call_output(error.message);
            }
        }
        let profile = std::env::var_os("MNCS_RUNTIME_PROFILE").is_some();
        let request_started = Instant::now();
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
            type_arguments: options.type_arguments.clone(),
            step_budget,
            policy: ExecutionPolicy::default(),
            host_grants: Vec::new(),
            call_depth_budget: None,
        };
        if !grants.is_empty() || provider_runtime.is_some() {
            request = request.with_host_grants(&grants);
            request.policy.effects = EffectExecutionPolicy::Realize;
        }
        let request_build_ns = request_started.elapsed().as_nanos();
        let execution_started = Instant::now();
        let observation = provider_runtime.map_or_else(
            || self.inner.execute(&request),
            |provider_runtime| self.inner.execute_with_provider(&request, provider_runtime),
        );
        let execution_ns = execution_started.elapsed().as_nanos();
        let result_started = Instant::now();
        let status = match observation.status {
            ExecutionStatus::Returned => "returned",
            ExecutionStatus::InvalidRequest => "invalid_request",
            ExecutionStatus::RuntimeFailure => "runtime_failure",
            ExecutionStatus::Unsupported => "unsupported",
            ExecutionStatus::BudgetExhausted => "budget_exhausted",
        }
        .to_owned();
        let output = CallOutput {
            status,
            returned: observation.returned,
            steps: observation.steps,
            effects: observation.effects,
            failure_reason: observation.failure.map(|failure| failure.reason),
            artifact_identity: self.artifact_identity().to_owned(),
            artifact_sha256: self.digest().to_owned(),
            backend: self.backend_name().to_owned(),
            reused_session: self.reused(),
            invoked_callable: None,
        };
        if profile {
            eprintln!(
                "mncs-embed-call-profile request_build_ns={} execution_ns={} result_build_ns={}",
                request_build_ns,
                execution_ns,
                result_started.elapsed().as_nanos()
            );
        }
        output
    }

    fn invalid_call_output(&self, message: String) -> CallOutput {
        CallOutput {
            status: "invalid_request".to_owned(),
            returned: Vec::new(),
            steps: 0,
            effects: Vec::new(),
            failure_reason: Some(message),
            artifact_identity: self.artifact_identity().to_owned(),
            artifact_sha256: self.digest().to_owned(),
            backend: self.backend_name().to_owned(),
            reused_session: self.reused(),
            invoked_callable: None,
        }
    }

    /// Execute one named entrypoint with a generic admitted provider
    /// registry. The consumer still supplies a concrete typed argument and
    /// receives the expected nominal result; provider selection never enters
    /// this API as a string or a family-specific branch.
    pub fn call_with_provider(
        &self,
        module: &str,
        function: &str,
        arguments: Vec<ExecutionValue>,
        options: &CallOptions,
        provider_runtime: &dyn mncs_model::ProviderRuntime,
    ) -> CallOutput {
        self.call_with_runtime(module, function, arguments, options, Some(provider_runtime))
    }

    /// Execute typed calls in order while retaining one verified session.
    /// Each output is independent and preserves its own status, effects, and
    /// artifact identity.  This is the reusable in-process capability used
    /// by native tooling; callers do not need a subprocess or JSON roundtrip
    /// to compose several MNCS calls.
    pub fn call_batch(&self, calls: &[BatchCall]) -> Vec<CallOutput> {
        calls
            .iter()
            .map(|call| match &call.target {
                BatchCallTarget::Named { module, function } => {
                    self.call(module, function, call.arguments.clone(), &call.options)
                }
                BatchCallTarget::Callable(reference) => self
                    .call_identity(reference, call.arguments.clone(), &call.options)
                    .unwrap_or_else(|error| self.invalid_call_output(error.message)),
            })
            .collect()
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

    /// JSON convenience for the name-oriented typed host boundary.  The
    /// values are resolved against the verified artifact's callable metadata;
    /// callers provide record/enum names and logical scalar values, never
    /// positional integer encodings or nominal identities.
    pub fn call_typed_json(
        &self,
        module: &str,
        function: &str,
        args_json: &str,
        options: &CallOptions,
    ) -> Result<CallOutput, EmbedError> {
        let values: Vec<HostExecutionValue> = serde_json::from_str(args_json).map_err(|error| {
            EmbedError::new(
                "bad_typed_arguments",
                format!("typed argument JSON rejected: {error}"),
            )
        })?;
        self.call_typed(module, function, values, options)
    }

    /// Name-oriented retained-call boundary for hosts that already decoded
    /// JSON. The canonical ABI values still come from the admitted artifact's
    /// interface, so nominal identities and scalar widths remain language
    /// owned.
    pub fn call_typed(
        &self,
        module: &str,
        function: &str,
        values: Vec<HostExecutionValue>,
        options: &CallOptions,
    ) -> Result<CallOutput, EmbedError> {
        if let Some(expected) = options.expected_interface_identity.as_deref() {
            let Some(actual) = self.interface_identity() else {
                return Err(EmbedError::new(
                    "stale_interface",
                    "artifact has no language-owned interface identity; regenerate the artifact",
                ));
            };
            if expected != actual {
                return Err(EmbedError::new(
                    "stale_interface",
                    format!(
                        "interface identity mismatch: expected {expected}, loaded {actual}; regenerate the host binding"
                    ),
                ));
            }
        }
        let arguments = mncs_codegen::resolve_typed_arguments_for_artifact(
            self.inner.artifact(),
            module,
            function,
            &options.type_arguments,
            &values,
        )
        .map_err(|error| EmbedError::new("bad_typed_arguments", error))?;
        Ok(self.call(module, function, arguments, options))
    }

    /// Name-oriented typed call through a generic admitted provider
    /// registry. Interface identity is checked before argument resolution,
    /// exactly as on [`Self::call_typed_json`].
    pub fn call_typed_json_with_provider(
        &self,
        module: &str,
        function: &str,
        args_json: &str,
        options: &CallOptions,
        provider_runtime: &dyn mncs_model::ProviderRuntime,
    ) -> Result<CallOutput, EmbedError> {
        if let Some(expected) = options.expected_interface_identity.as_deref() {
            let Some(actual) = self.interface_identity() else {
                return Err(EmbedError::new(
                    "stale_interface",
                    "artifact has no language-owned interface identity; regenerate the host binding",
                ));
            };
            if expected != actual {
                return Err(EmbedError::new(
                    "stale_interface",
                    format!(
                        "interface identity mismatch: expected {expected}, loaded {actual}; regenerate the host binding"
                    ),
                ));
            }
        }
        let values: Vec<HostExecutionValue> = serde_json::from_str(args_json).map_err(|error| {
            EmbedError::new(
                "bad_typed_arguments",
                format!("typed argument JSON rejected: {error}"),
            )
        })?;
        let arguments = mncs_codegen::resolve_typed_arguments_for_artifact(
            self.inner.artifact(),
            module,
            function,
            &options.type_arguments,
            &values,
        )
        .map_err(|error| EmbedError::new("bad_typed_arguments", error))?;
        Ok(self.call_with_provider(module, function, arguments, options, provider_runtime))
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
    let profile = std::env::var_os("MNCS_RUNTIME_PROFILE").is_some();
    let decode_started = Instant::now();
    let artifact = match Artifact::from_json(raw) {
        Ok(artifact) => artifact,
        Err(error) => {
            stash_error(error.to_string());
            return ptr::null_mut();
        }
    };
    if profile {
        eprintln!(
            "mncs-embed-profile phase=artifact_admission elapsed_ns={}",
            decode_started.elapsed().as_nanos()
        );
    }
    let open_started = Instant::now();
    let opened = Session::open(artifact).map_err(|error| error.to_string());
    if profile {
        eprintln!(
            "mncs-embed-profile phase=session_open elapsed_ns={}",
            open_started.elapsed().as_nanos()
        );
    }
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
    // The single-call C boundary stays concrete-only (stable signature):
    // generic entrypoints go through the batch API's per-request
    // `type_arguments` or the Rust `Session::call`.
    let options = CallOptions {
        step_budget,
        grants,
        expected_interface_identity: None,
        type_arguments: Vec::new(),
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
/// `{module, function, args|typed_args, ...}` or
/// `{callable_reference, args|typed_args, ...}` objects. Identity references
/// bind the callable, declaration, signature, and exact artifact revision;
/// generic `type_arguments` select an artifact-compiled specialization just
/// like `ExecutionRequest.type_arguments`. Returns the
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
    let profile = std::env::var_os("MNCS_RUNTIME_PROFILE").is_some();
    let abi_copy_started = Instant::now();
    let text = read_c_str(requests_json).unwrap_or_else(|| "[]".to_owned());
    if profile {
        eprintln!(
            "mncs-embed-profile phase=request_abi_copy elapsed_ns={}",
            abi_copy_started.elapsed().as_nanos()
        );
    }
    #[derive(serde::Deserialize)]
    struct BatchRequest {
        #[serde(default)]
        module: Option<String>,
        #[serde(default)]
        function: Option<String>,
        #[serde(default)]
        callable_reference: Option<CallableReference>,
        #[serde(default)]
        args: Option<Vec<mncs_model::ExecutionValue>>,
        #[serde(default)]
        typed_args: Option<Vec<HostExecutionValue>>,
        #[serde(default)]
        grants: Vec<Grant>,
        #[serde(default)]
        step_budget: u64,
        #[serde(default)]
        type_arguments: Vec<mncs_model::ExecutionTypeArgument>,
    }
    let request_decode_started = Instant::now();
    let requests: Vec<BatchRequest> = match serde_json::from_str(&text) {
        Ok(requests) => requests,
        Err(error) => {
            stash_error(format!("batch request JSON rejected: {error}"));
            return ptr::null_mut();
        }
    };
    if profile {
        eprintln!(
            "mncs-embed-profile phase=request_json_decode elapsed_ns={}",
            request_decode_started.elapsed().as_nanos()
        );
    }
    let execution_started = Instant::now();
    let outputs: Vec<CallOutput> = requests
        .into_iter()
        .map(|request| {
            let options = CallOptions {
                step_budget: request.step_budget,
                grants: request.grants,
                expected_interface_identity: None,
                type_arguments: request.type_arguments,
            };
            let named_target = match (request.module, request.function) {
                (Some(module), Some(function)) => Some((module, function)),
                (None, None) => None,
                _ => {
                    return session.invalid_call_output(
                        "batch request must provide both module and function".to_owned(),
                    );
                }
            };
            if named_target.is_some() == request.callable_reference.is_some() {
                return session.invalid_call_output(
                    "batch request must provide exactly one of a named target or callable_reference"
                        .to_owned(),
                );
            }
            let run_raw =
                |arguments| match (named_target.as_ref(), request.callable_reference.as_ref()) {
                    (Some((module, function)), None) => {
                        session.call(module, function, arguments, &options)
                    }
                    (None, Some(reference)) => session
                        .call_identity(reference, arguments, &options)
                        .unwrap_or_else(|error| session.invalid_call_output(error.message)),
                    _ => unreachable!("target shape validated above"),
                };
            let run_typed =
                |arguments| match (named_target.as_ref(), request.callable_reference.as_ref()) {
                    (Some((module, function)), None) => session
                        .call_typed(module, function, arguments, &options)
                        .unwrap_or_else(|error| session.invalid_call_output(error.message)),
                    (None, Some(reference)) => session
                        .call_identity_typed(reference, arguments, &options)
                        .unwrap_or_else(|error| session.invalid_call_output(error.message)),
                    _ => unreachable!("target shape validated above"),
                };
            match (request.args, request.typed_args) {
                (Some(_), Some(_)) => session.invalid_call_output(
                    "batch request must provide either args or typed_args, not both".to_owned(),
                ),
                (Some(arguments), None) => run_raw(arguments),
                (None, Some(arguments)) => run_typed(arguments),
                (None, None) => run_raw(Vec::new()),
            }
        })
        .collect();
    if profile {
        eprintln!(
            "mncs-embed-profile phase=request_validation_and_execution elapsed_ns={}",
            execution_started.elapsed().as_nanos()
        );
    }
    let response_encode_started = Instant::now();
    match CallResponse::of(&outputs) {
        Ok(response) => {
            if profile {
                eprintln!(
                    "mncs-embed-profile phase=result_json_encode elapsed_ns={}",
                    response_encode_started.elapsed().as_nanos()
                );
            }
            response
        }
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
