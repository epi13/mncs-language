//! Tranche-0.2 proof admission: file artifact to corroborated relationship.
//!
//! This module is TRANSPORT and EXECUTION GLUE, not proof authority:
//!
//! - Host Rust reads artifact bytes/files and decodes JSON (transport).
//! - Semantic admission is performed by EXECUTING the MNCS admission module
//!   (`library/core/proof_admit.mncs`, `mncs.core.proof_admit.v1`) in-process
//!   over the exact artifact cells. The MNCS-issued verdict and canonical
//!   assumption set are decoded from the returned `ProofBinding` record.
//! - Proof validity is never decided here: [`admit_artifact`] refuses
//!   authority (returns `Err`) on any invalid proof, non-PASS verdict,
//!   stale binding, wrong kernel, wrong obligation, or changed assumptions.
//!   Corroboration by the independent checker decides only whether the
//!   MNCS-issued binding may be CONSUMED, never whether it is valid.
//!
//! The authority chain is: candidate artifact -> MNCS kernel execution ->
//! MNCS PASS/FAIL/UNKNOWN -> sealed binding -> independent corroboration ->
//! consumable relationship. Every arrow is asserted by tests in this module
//! and by `crates/mncs-cli/tests/proof_dep_admission.rs`.

use std::fmt;
use std::path::{Path, PathBuf};

use mncs_model::{
    corroborate_proof, dep_check, BodyExecutionSession, DepArtifact, DepAssumptionSet, DepCell,
    DepCorroboration, DepTag, DepVerdict, ExecutionRequest, ExecutionStatus, ExecutionTarget,
    ExecutionValue, IntegerType, ProofRelationship, EXECUTION_REQUEST_SCHEMA_VERSION,
    PROOF_DEP_KERNEL_ID,
};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

use crate::frontend::{elaborate_program_with_resolver, ModuleResolver};

/// Step budget for `seal_binding` executions (host policy, not semantics:
/// sealing runs the kernel check plus a full assumption-set recomputation).
pub const ADMISSION_SEAL_BUDGET: u64 = 4_000_000;
/// Step budget for `binding_reusable` executions (re-runs the kernel over
/// the sealed cells plus exact comparisons).
pub const ADMISSION_REUSE_BUDGET: u64 = 6_000_000;
/// Maximum dependency fingerprint slots bound into one admission.
pub const ADMISSION_MAX_DEPENDENCIES: usize = 4;

/// How [`admit_artifact`] can refuse authority. Every variant means "no
/// binding is issued and nothing may be consumed"; none of them invents an
/// authority rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    InvalidArtifact(String),
    StaleIdentity,
    WrongKernel { expected: String, observed: String },
    MalformedIdentity,
    MalformedBuffer,
    AdmissionModule(String),
    ExecutionFailed { status: String, reason: String },
    DecodeFailed(String),
    IdentityEchoMismatch(String),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArtifact(reason) => write!(f, "invalid proof artifact: {reason}"),
            Self::StaleIdentity => write!(f, "artifact seal does not cover its bytes"),
            Self::WrongKernel { expected, observed } => write!(
                f,
                "artifact kernel {observed:?} is not the admission kernel {expected:?}"
            ),
            Self::MalformedIdentity => write!(f, "artifact identity is not a sealed digest"),
            Self::MalformedBuffer => write!(f, "artifact cells do not decode to a buffer"),
            Self::AdmissionModule(reason) => write!(f, "admission module unavailable: {reason}"),
            Self::ExecutionFailed { status, reason } => {
                write!(f, "MNCS admission execution {status}: {reason}")
            }
            Self::DecodeFailed(reason) => write!(f, "cannot decode MNCS binding: {reason}"),
            Self::IdentityEchoMismatch(field) => write!(
                f,
                "MNCS binding echoes different {field} than presented; refusing"
            ),
        }
    }
}

impl std::error::Error for AdmissionError {}

/// An MNCS-issued verdict with its canonical assumption set, the
/// corroboration report, and the execution evidence behind both. The live
/// artifact travels with the admission so consumption re-presents the exact
/// sealed cells (never a re-serialized copy).
#[derive(Debug, Clone)]
pub struct AdmittedProof {
    pub relationship: ProofRelationship,
    pub corroboration: DepCorroboration,
    pub program_fingerprint: String,
    pub seal_steps: u64,
    pub artifact: DepArtifact,
}

// --- Value encoding (transport): DepCell to MNCS Cell record ----------
// Identity constants mirror `scripts/gen_proof_dep_corpus.py`; the
// `cell_encoding_matches_checked_in_corpus` test pins them against the
// checked-in corpus so drift fails loudly instead of silently.

const CELL_IDENTITY: &str = "mncs:0.2:record-type:mncs.core.proof_dep.v2::Cell::arg0%3Ai64%3Barg1%3Ai64%3Barg2%3Ai64%3Barg3%3Ai64%3Btag%3ATermTag%3B";
const TAG_IDENTITY: &str = "mncs:0.2:finite-type:mncs.core.proof_dep.v2::TermTag";
const TAG_VARIANT_PREFIX: &str = "mncs:0.2:finite-variant:mncs.core.proof_dep.v2::TermTag::";

/// Corpus order (also the finite discriminant order): Universe first,
/// Unsupported last.
const TAG_ORDER: [&str; 15] = [
    "Universe",
    "Var",
    "Pi",
    "Lam",
    "App",
    "Nat",
    "Zero",
    "Succ",
    "Plus",
    "NatElim",
    "Eq",
    "Refl",
    "Hyp",
    "Cong",
    "Unsupported",
];

fn dep_tag_name(tag: DepTag) -> &'static str {
    match tag {
        DepTag::Universe => "Universe",
        DepTag::Var => "Var",
        DepTag::Pi => "Pi",
        DepTag::Lam => "Lam",
        DepTag::App => "App",
        DepTag::Nat => "Nat",
        DepTag::Zero => "Zero",
        DepTag::Succ => "Succ",
        DepTag::Plus => "Plus",
        DepTag::NatElim => "NatElim",
        DepTag::Eq => "Eq",
        DepTag::Refl => "Refl",
        DepTag::Hyp => "Hyp",
        DepTag::Cong => "Cong",
        DepTag::Unsupported => "Unsupported",
    }
}

fn i64_value(value: i64) -> ExecutionValue {
    ExecutionValue::Integer {
        value: value as i128,
        ty: IntegerType {
            bits: 64,
            signed: true,
        },
    }
}

fn u64_value(value: u64) -> ExecutionValue {
    ExecutionValue::Integer {
        value: value as i128,
        ty: IntegerType {
            bits: 64,
            signed: false,
        },
    }
}

fn byte_value(value: u8) -> ExecutionValue {
    ExecutionValue::Byte {
        value: value as i128,
    }
}

fn dep_cell_value(cell: &DepCell) -> ExecutionValue {
    let name = dep_tag_name(cell.tag);
    let discriminant = TAG_ORDER
        .iter()
        .position(|candidate| *candidate == name)
        .expect("every DepTag has a corpus name") as u32;
    ExecutionValue::Record {
        type_identity: mncs_model::SemanticId(CELL_IDENTITY.to_owned()),
        name: "Cell".to_owned(),
        fields: vec![
            ("arg0".to_owned(), i64_value(cell.args[0])),
            ("arg1".to_owned(), i64_value(cell.args[1])),
            ("arg2".to_owned(), i64_value(cell.args[2])),
            ("arg3".to_owned(), i64_value(cell.args[3])),
            (
                "tag".to_owned(),
                ExecutionValue::Finite {
                    type_identity: mncs_model::SemanticId(TAG_IDENTITY.to_owned()),
                    variant_identity: mncs_model::SemanticId(format!("{TAG_VARIANT_PREFIX}{name}")),
                    discriminant,
                    payload: Default::default(),
                },
            ),
        ]
        .into(),
    }
}

fn bytes_value(raw: &[u8], width: usize) -> ExecutionValue {
    let mut values = Vec::with_capacity(width);
    for slot in 0..width {
        values.push(byte_value(*raw.get(slot).unwrap_or(&0)));
    }
    ExecutionValue::Sequence {
        values: values.into(),
    }
}

// --- Library resolution (transport): find admission sources ----------

struct LibraryResolver {
    roots: Vec<PathBuf>,
}

impl LibraryResolver {
    fn candidates(root: &Path, module: &str) -> Vec<PathBuf> {
        let dotted = module.replace('.', "/");
        let tail = module.rsplit('.').next().unwrap_or(module);
        let mut paths = vec![root.join(format!("{dotted}.mncs"))];
        if let Some((head, _)) = split_version_tail(module) {
            paths.push(root.join(format!("{}.mncs", head.replace('.', "/"))));
            if let Some(stripped) = head.strip_prefix("mncs.") {
                paths.push(root.join(format!("{}.mncs", stripped.replace('.', "/"))));
            }
        }
        if let Some(rest) = module.strip_prefix("mncs.") {
            paths.push(root.join(format!("{}.mncs", rest.replace('.', "/"))));
        }
        paths.push(root.join(format!("{tail}.mncs")));
        paths
    }
}

fn split_version_tail(module: &str) -> Option<(&str, &str)> {
    let (head, last) = module.rsplit_once('.')?;
    let digits = last.strip_prefix('v')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if head.is_empty() || head.contains("..") {
        return None;
    }
    Some((head, last))
}

impl ModuleResolver for LibraryResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        for root in &self.roots {
            for candidate in Self::candidates(root, module) {
                if let Ok(text) = std::fs::read_to_string(&candidate) {
                    return Some(SourceEnvelope::inline(
                        SourceArtifactKind::Program,
                        candidate.display().to_string(),
                        text,
                    ));
                }
            }
        }
        None
    }
}

/// Library roots: explicit directories first, then `MNCS_LIBRARY_PATH`.
pub fn admission_library_roots(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = extra.to_vec();
    if let Ok(path) = std::env::var("MNCS_LIBRARY_PATH") {
        roots.extend(path.split(':').filter(|e| !e.is_empty()).map(PathBuf::from));
    }
    roots
}

fn load_admission_program(
    library_roots: &[PathBuf],
) -> Result<(mncs_model::Program, String), AdmissionError> {
    let resolver = LibraryResolver {
        roots: library_roots.to_vec(),
    };
    let source = resolver
        .resolve("mncs.core.proof_admit.v1")
        .ok_or_else(|| {
            AdmissionError::AdmissionModule(
                "mncs.core.proof_admit.v1 not found under library roots".to_owned(),
            )
        })?;
    let parsed = parse(&source);
    if !parsed.is_valid() {
        return Err(AdmissionError::AdmissionModule(format!(
            "admission source fails to parse: {:?}",
            parsed.diagnostics
        )));
    }
    let ast = parsed
        .ast
        .ok_or_else(|| AdmissionError::AdmissionModule("admission source has no AST".to_owned()))?;
    let (program, _) = elaborate_program_with_resolver(&ast, &resolver);
    let program = program.map_err(|diagnostics| {
        AdmissionError::AdmissionModule(format!(
            "admission source fails to elaborate: {diagnostics:?}"
        ))
    })?;
    if !program.validate().valid {
        return Err(AdmissionError::AdmissionModule(
            "admission program fails validation".to_owned(),
        ));
    }
    let fingerprint = program.content_fingerprint().map_err(|error| {
        AdmissionError::AdmissionModule(format!("admission program not fingerprintable: {error}"))
    })?;
    Ok((program, fingerprint))
}

// --- Digest and fingerprint handling (transport) -----------------------

fn artifact_digest(artifact: &DepArtifact) -> Result<[u8; 32], AdmissionError> {
    let hex = artifact
        .identity
        .strip_prefix("mncs:proof-dep:")
        .ok_or(AdmissionError::MalformedIdentity)?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AdmissionError::MalformedIdentity);
    }
    let mut digest = [0u8; 32];
    for (slot, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| AdmissionError::MalformedIdentity)?;
        digest[slot] =
            u8::from_str_radix(text, 16).map_err(|_| AdmissionError::MalformedIdentity)?;
    }
    Ok(digest)
}

/// sha256 of the UTF-8 spelling, always hashed (never hex-decoded): the
/// canonical digest mapping for variable-length identities (obligations).
/// Transport canonicalization, not semantics: MNCS compares bytes exactly.
pub fn obligation_digest(obligation: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(obligation.as_bytes());
    hasher.finalize().into()
}

/// Normalize one dependency fingerprint string to 32 bytes for a slot:
/// 64-hex decodes directly, anything else hashes (sha256 of the UTF-8
/// spelling). Normalization is transport canonicalization, not semantics:
/// MNCS compares the resulting bytes exactly.
pub fn normalize_dependency_fingerprint(text: &str) -> [u8; 32] {
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut digest = [0u8; 32];
        let mut ok = true;
        for (slot, chunk) in text.as_bytes().chunks(2).enumerate() {
            match std::str::from_utf8(chunk)
                .ok()
                .and_then(|part| u8::from_str_radix(part, 16).ok())
            {
                Some(byte) => digest[slot] = byte,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return digest;
        }
    }
    obligation_digest(text)
}

fn seal_argument_cells(artifact: &DepArtifact) -> Result<Vec<ExecutionValue>, AdmissionError> {
    let buffer = artifact.buffer().ok_or(AdmissionError::MalformedBuffer)?;
    let mut values: Vec<ExecutionValue> = buffer.cells.iter().map(dep_cell_value).collect();
    let pad = DepCell::new(DepTag::Nat, [0, 0, 0, 0]);
    while values.len() < mncs_model::PROOF_DEP_BUFFER_CAPACITY {
        values.push(dep_cell_value(&pad));
    }
    if values.len() != mncs_model::PROOF_DEP_BUFFER_CAPACITY {
        return Err(AdmissionError::MalformedBuffer);
    }
    Ok(values)
}

// --- Binding record decoding (transport) -------------------------------

fn record_field<'a>(value: &'a ExecutionValue, name: &str) -> Option<&'a ExecutionValue> {
    match value {
        ExecutionValue::Record { fields, .. } => {
            fields.iter().find(|(key, _)| key == name).map(|(_, v)| v)
        }
        _ => None,
    }
}

fn decode_integer(value: &ExecutionValue) -> Option<i128> {
    match value {
        ExecutionValue::Integer { value, .. } => Some(*value),
        _ => None,
    }
}

fn decode_byte_sequence(value: &ExecutionValue) -> Option<Vec<u8>> {
    match value {
        ExecutionValue::Sequence { values } => values
            .iter()
            .map(|item| match item {
                ExecutionValue::Byte { value } => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn decode_integer_sequence(value: &ExecutionValue) -> Option<Vec<i64>> {
    match value {
        ExecutionValue::Sequence { values } => values
            .iter()
            .map(|item| match item {
                ExecutionValue::Integer { value, .. } => i64::try_from(*value).ok(),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn decode_boolean(value: &ExecutionValue) -> Option<bool> {
    match value {
        ExecutionValue::Boolean { value } => Some(*value),
        _ => None,
    }
}

fn decode_assumption_set(value: &ExecutionValue) -> Option<DepAssumptionSet> {
    let count = usize::try_from(decode_integer(record_field(value, "count")?)?).ok()?;
    let valid = decode_boolean(record_field(value, "valid")?)?;
    let hyp = decode_integer_sequence(record_field(value, "hyp")?)?;
    let level = decode_integer_sequence(record_field(value, "level")?)?;
    let carrier = decode_integer_sequence(record_field(value, "carrier")?)?;
    if hyp.len() < count || level.len() < count || carrier.len() < count {
        return None;
    }
    let uses = (0..count)
        .map(|slot| mncs_model::DepAssumptionUse {
            hyp: hyp[slot],
            level: level[slot],
            carrier: carrier[slot],
        })
        .collect();
    Some(DepAssumptionSet { uses, valid })
}

struct DecodedBinding {
    verdict: DepVerdict,
    assumptions: DepAssumptionSet,
    proof: Vec<u8>,
    obligation: Vec<u8>,
    kernel: Vec<u8>,
    kernel_len: usize,
}

fn decode_binding(value: &ExecutionValue) -> Option<DecodedBinding> {
    let verdict = DepVerdict::from_code(decode_integer(record_field(value, "verdict")?)? as i64)?;
    let assumptions = decode_assumption_set(record_field(value, "assumptions")?)?;
    let proof = decode_byte_sequence(record_field(value, "proof")?)?;
    let obligation = decode_byte_sequence(record_field(value, "obligation")?)?;
    let kernel = decode_byte_sequence(record_field(value, "kernel")?)?;
    let kernel_len = usize::try_from(decode_integer(record_field(value, "kernel_len")?)?).ok()?;
    if proof.len() != 32 || obligation.len() != 32 || kernel.len() != 32 {
        return None;
    }
    Some(DecodedBinding {
        verdict,
        assumptions,
        proof,
        obligation,
        kernel,
        kernel_len,
    })
}

fn execute_admit(
    program: &mncs_model::Program,
    function: &str,
    arguments: Vec<ExecutionValue>,
    budget: u64,
) -> Result<(ExecutionValue, u64), AdmissionError> {
    let request = ExecutionRequest {
        schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
        target: ExecutionTarget {
            module: "mncs.core.proof_admit.v1".to_owned(),
            function: function.to_owned(),
        },
        arguments,
        // Admission executes concrete admission entrypoints only.
        type_arguments: Vec::new(),
        step_budget: budget,
        policy: Default::default(),
        host_grants: Vec::new(),
        call_depth_budget: None,
    };
    let result = BodyExecutionSession::new(program).execute(&request);
    if result.status != ExecutionStatus::Returned {
        return Err(AdmissionError::ExecutionFailed {
            status: format!("{:?}", result.status),
            reason: result
                .failure
                .map(|failure| failure.reason)
                .unwrap_or_else(|| "no reason recorded".to_owned()),
        });
    }
    let value = result.returned.into_iter().next().ok_or_else(|| {
        AdmissionError::DecodeFailed("admission execution returned no value".to_owned())
    })?;
    Ok((value, result.steps))
}

// --- Admission: artifact file to corroborated relationship -------------

fn seal_arguments(
    artifact: &DepArtifact,
    digest: &[u8; 32],
    obligation: &[u8; 32],
    dependencies: &[[u8; 32]],
) -> Result<Vec<ExecutionValue>, AdmissionError> {
    let mut slots = [[0u8; 32]; 4];
    for (slot, fingerprint) in dependencies.iter().take(4).enumerate() {
        slots[slot] = *fingerprint;
    }
    Ok(vec![
        bytes_value(digest, 32),
        bytes_value(obligation, 32),
        bytes_value(PROOF_DEP_KERNEL_ID.as_bytes(), 32),
        u64_value(PROOF_DEP_KERNEL_ID.len() as u64),
        ExecutionValue::Sequence {
            values: seal_argument_cells(artifact)?.into(),
        },
        byte_value(artifact.count as u8),
        byte_value(artifact.proof as u8),
        byte_value(artifact.proposition as u8),
        u64_value(dependencies.len().min(4) as u64),
        bytes_value(&slots[0], 32),
        bytes_value(&slots[1], 32),
        bytes_value(&slots[2], 32),
        bytes_value(&slots[3], 32),
    ])
}

/// Admit one proof artifact through the real toolchain path.
///
/// 1. Transport: parse the artifact JSON (byte/file handling only).
/// 2. Seal check: the identity must cover the exact bytes presented.
/// 3. Semantic admission: execute MNCS `seal_binding` over the exact cells;
///    the verdict and canonical set are MNCS-issued.
/// 4. Corroboration: the independent checker recomputes over the same cells.
/// 5. The relationship is consumable only on MNCS PASS with full agreement.
///
/// `dependencies` are the slot fingerprints the admission seals under (at
/// most four; slot 3 is reserved zero — pass at most three meaningful
/// fingerprints). Any failure returns `Err`: no binding, no consumption.
pub fn admit_artifact(
    artifact_json: &str,
    dependencies: &[[u8; 32]],
    library_roots: &[PathBuf],
) -> Result<AdmittedProof, AdmissionError> {
    if dependencies.len() > ADMISSION_MAX_DEPENDENCIES {
        return Err(AdmissionError::InvalidArtifact(format!(
            "too many dependency slots: {}",
            dependencies.len()
        )));
    }
    let artifact: DepArtifact = serde_json::from_str(artifact_json).map_err(|error| {
        AdmissionError::InvalidArtifact(format!("artifact JSON does not parse: {error}"))
    })?;
    if !artifact.identity_is_valid() {
        return Err(AdmissionError::StaleIdentity);
    }
    if artifact.kernel != PROOF_DEP_KERNEL_ID {
        return Err(AdmissionError::WrongKernel {
            expected: PROOF_DEP_KERNEL_ID.to_owned(),
            observed: artifact.kernel.clone(),
        });
    }
    // The obligation identity crosses into MNCS as its sha256 digest (real
    // obligation identities exceed every bounded byte slot; exactness rests
    // on the digest, as with artifact seals).
    let obligation = obligation_digest(&artifact.obligation);
    let digest = artifact_digest(&artifact)?;
    let buffer = artifact.buffer().ok_or(AdmissionError::MalformedBuffer)?;
    let (program, program_fingerprint) = load_admission_program(library_roots)?;
    let arguments = seal_arguments(&artifact, &digest, &obligation, dependencies)?;
    let (binding_value, seal_steps) =
        execute_admit(&program, "seal_binding", arguments, ADMISSION_SEAL_BUDGET)?;
    let decoded = decode_binding(&binding_value)
        .ok_or_else(|| AdmissionError::DecodeFailed("seal record shape".to_owned()))?;
    // Echo check: MNCS must have sealed exactly what was presented.
    if decoded.proof != digest {
        return Err(AdmissionError::IdentityEchoMismatch(
            "proof digest".to_owned(),
        ));
    }
    if decoded.obligation != obligation {
        return Err(AdmissionError::IdentityEchoMismatch(
            "obligation".to_owned(),
        ));
    }
    if decoded.kernel_len != PROOF_DEP_KERNEL_ID.len()
        || decoded.kernel.get(..decoded.kernel_len) != Some(PROOF_DEP_KERNEL_ID.as_bytes())
    {
        return Err(AdmissionError::IdentityEchoMismatch("kernel".to_owned()));
    }
    let rust_verdict = DepVerdict::from_code(dep_check(
        buffer.cells.clone(),
        artifact.count,
        artifact.proof,
        artifact.proposition,
    ));
    let corroboration = corroborate_proof(
        decoded.verdict,
        Some(&decoded.assumptions),
        buffer.cells,
        artifact.count,
        artifact.proof,
        artifact.proposition,
        rust_verdict,
    );
    let consumable = decoded.verdict == DepVerdict::Pass && corroboration.consumable();
    if !consumable {
        return Err(match corroboration {
            DepCorroboration::AgreeNonPass { .. } => AdmissionError::ExecutionFailed {
                status: "Returned".to_owned(),
                reason: format!("MNCS verdict {:?} authorizes nothing", decoded.verdict),
            },
            DepCorroboration::Disagree { mncs, rust } => AdmissionError::ExecutionFailed {
                status: "Returned".to_owned(),
                reason: format!("checker dispute MNCS {mncs:?} vs Rust {rust:?}; quarantined"),
            },
            DepCorroboration::CheckerUnavailable => AdmissionError::ExecutionFailed {
                status: "Returned".to_owned(),
                reason: "independent checker unavailable; quarantined".to_owned(),
            },
            DepCorroboration::AgreePass => AdmissionError::ExecutionFailed {
                status: "Returned".to_owned(),
                reason: "internal corroboration mismatch".to_owned(),
            },
        });
    }
    let relationship = ProofRelationship {
        schema_version: mncs_model::PROOF_RELATIONSHIP_SCHEMA_VERSION.to_owned(),
        proof_identity: artifact.identity.clone(),
        obligation: artifact.obligation.clone(),
        kernel: artifact.kernel.clone(),
        assumptions: decoded.assumptions,
        mncs_verdict: decoded.verdict,
        sealed_binding: binding_value,
        admission: mncs_model::ProofAdmissionEvidence {
            program_fingerprint: program_fingerprint.clone(),
            seal_steps,
        },
        corroboration,
        dependencies: dependencies
            .iter()
            .map(|slot| {
                slot.iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            })
            .collect(),
    };
    Ok(AdmittedProof {
        relationship,
        corroboration,
        program_fingerprint,
        seal_steps,
        artifact,
    })
}

/// Re-validate one admitted relationship against the exact current use by
/// executing MNCS `binding_reusable`. The presented cells, indices,
/// identities, and slot fingerprints must match the sealed binding, the
/// kernel must re-verify PASS over the sealed cells, and the canonical set
/// must recompute exactly. Returns the MNCS boolean: `true` authorizes this
/// one use, `false` (or any `Err`) withholds it.
#[allow(clippy::too_many_arguments)]
pub fn authorize_reuse(
    admitted: &AdmittedProof,
    artifact: &DepArtifact,
    dependencies: &[[u8; 32]],
    library_roots: &[PathBuf],
) -> Result<bool, AdmissionError> {
    if dependencies.len() > ADMISSION_MAX_DEPENDENCIES {
        return Err(AdmissionError::InvalidArtifact(format!(
            "too many dependency slots: {}",
            dependencies.len()
        )));
    }
    if !artifact.identity_is_valid() {
        return Err(AdmissionError::StaleIdentity);
    }
    if artifact.identity != admitted.relationship.proof_identity {
        return Err(AdmissionError::IdentityEchoMismatch(
            "proof digest".to_owned(),
        ));
    }
    let digest = artifact_digest(artifact)?;
    let obligation = obligation_digest(&artifact.obligation);
    let (program, _) = load_admission_program(library_roots)?;
    let mut slots = [[0u8; 32]; 4];
    for (slot, fingerprint) in dependencies.iter().take(4).enumerate() {
        slots[slot] = *fingerprint;
    }
    let arguments = vec![
        admitted.relationship.sealed_binding.clone(),
        bytes_value(&digest, 32),
        bytes_value(&obligation, 32),
        bytes_value(PROOF_DEP_KERNEL_ID.as_bytes(), 32),
        u64_value(PROOF_DEP_KERNEL_ID.len() as u64),
        ExecutionValue::Sequence {
            values: seal_argument_cells(artifact)?.into(),
        },
        byte_value(artifact.count as u8),
        byte_value(artifact.proof as u8),
        byte_value(artifact.proposition as u8),
        u64_value(dependencies.len().min(4) as u64),
        bytes_value(&slots[0], 32),
        bytes_value(&slots[1], 32),
        bytes_value(&slots[2], 32),
        bytes_value(&slots[3], 32),
    ];
    let (value, _) = execute_admit(
        &program,
        "binding_reusable",
        arguments,
        ADMISSION_REUSE_BUDGET,
    )?;
    match value {
        ExecutionValue::Boolean { value } => Ok(value),
        _ => Err(AdmissionError::DecodeFailed(
            "binding_reusable did not return a boolean".to_owned(),
        )),
    }
}

/// One proof use requested for a compilation: the artifact file bytes plus
/// the SSA operation the use carries evidence for. `operation` may be `None`
/// to bind the proof to the single SSA operation carrying the artifact's
/// obligation (zero or several carriers is an error: the binding must be
/// unambiguous). The proof is recorded as validated formal evidence for
/// that operation; it does NOT discharge the obligation (discharge remains
/// future work and is never claimed here).
#[derive(Debug, Clone)]
pub struct CompilerProofInput {
    pub artifact_json: String,
    pub operation: Option<String>,
}

/// HIR and SSA lowered with admitted proofs attached and proof-bearing
/// evidence recorded, plus non-fatal proof diagnostics (withheld evidence).
pub struct ProofLowering {
    pub hir: mncs_model::HighLevelIr,
    pub ssa: mncs_model::SsaModule,
    pub diagnostics: Vec<mncs_model::CompilerDiagnostic>,
}

fn hex_slot(slot: &[u8; 32]) -> String {
    slot.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn operation_present(ssa: &mncs_model::SsaModule, operation: &str) -> bool {
    ssa.functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| block.instructions.iter())
        .any(|instruction| {
            instruction
                .semantic_identity
                .as_ref()
                .is_some_and(|identity| identity.0 == operation)
        })
}

/// Lower a program with admitted tranche-0.2 proofs carried through every
/// stage: proof-free HIR/SSA supply the dependency fingerprints, each
/// artifact is admitted under those exact fingerprints (genuine MNCS
/// execution plus independent corroboration), relationships attach to HIR
/// then SSA with re-validation, and each use is authorized by a fresh MNCS
/// `binding_reusable` execution before its proof-bearing evidence record is
/// written into the SSA transformations that flow to lowering evidence.
///
/// Fail-closed: admission refusal, unknown operations, attachment refusal,
/// and authorization errors fail the whole lowering. A clean MNCS `false`
/// (stale use) withholds the evidence record and continues with a
/// diagnostic instead.
pub fn lower_with_proofs(
    program: &mncs_model::Program,
    inputs: &[CompilerProofInput],
    library_roots: &[PathBuf],
) -> Result<ProofLowering, Box<mncs_model::CompilerDiagnostic>> {
    let fail = |code: &str,
                kind: mncs_model::CompilerDiagnosticKind,
                message: String|
     -> mncs_model::CompilerDiagnostic {
        mncs_model::CompilerDiagnostic::new(code, kind, message)
    };
    use mncs_model::CompilerDiagnosticKind::{InternalCompilerDefect, InvalidRequest};
    // Phase 1: proof-free lowering supplies the dependency fingerprints
    // (slot 2) and the operation inventory.
    let hir_free = program.lower_to_ir().map_err(|error| {
        fail(
            "CMP204",
            InternalCompilerDefect,
            format!("validated semantic input failed HIR lowering: {error}"),
        )
    })?;
    let ssa_free = program.lower_to_ssa_from_ir(&hir_free).map_err(|error| {
        fail(
            "CMP205",
            InternalCompilerDefect,
            format!("validated semantic input failed SSA lowering: {error}"),
        )
    })?;
    let hir_fingerprint = hir_free.content_fingerprint.clone();
    // Phase 2: admit each artifact under the exact slot fingerprints.
    struct LiveUse {
        operation: String,
        admitted: AdmittedProof,
        deps_bytes: Vec<[u8; 32]>,
        deps_hex: Vec<String>,
    }
    let mut live = Vec::with_capacity(inputs.len());
    for input in inputs {
        let artifact: DepArtifact =
            serde_json::from_str(&input.artifact_json).map_err(|error| {
                fail(
                    "CMP206",
                    InvalidRequest,
                    format!("proof artifact JSON does not parse: {error}"),
                )
            })?;
        // Resolve the operation: explicit, or the single SSA operation
        // carrying the artifact's obligation (verified present below).
        let operation = match &input.operation {
            Some(operation) => operation.clone(),
            None => {
                let mut carriers = Vec::new();
                for function in &ssa_free.functions {
                    for block in &function.blocks {
                        for instruction in &block.instructions {
                            if instruction
                                .obligations
                                .iter()
                                .any(|obligation| obligation.0 == artifact.obligation)
                            {
                                if let Some(identity) = &instruction.semantic_identity {
                                    carriers.push(identity.0.clone());
                                }
                            }
                        }
                    }
                }
                carriers.sort();
                carriers.dedup();
                match carriers.len() {
                    1 => carriers.into_iter().next().expect("one carrier"),
                    0 => {
                        return Err(Box::new(fail(
                            "CMP206",
                            InvalidRequest,
                            format!(
                                "no SSA operation carries obligation {:?}; pass an explicit operation",
                                artifact.obligation
                            ),
                        )));
                    }
                    _ => {
                        return Err(Box::new(fail(
                            "CMP206",
                            InvalidRequest,
                            format!(
                                "obligation {:?} is carried by {} operations; pass an explicit operation",
                                artifact.obligation,
                                carriers.len()
                            ),
                        )));
                    }
                }
            }
        };
        if !operation_present(&ssa_free, &operation) {
            return Err(Box::new(fail(
                "CMP206",
                InvalidRequest,
                format!("proof use names an SSA operation absent from this program: {operation:?}"),
            )));
        }
        let slots = [
            normalize_dependency_fingerprint(&artifact.obligation),
            normalize_dependency_fingerprint(&operation),
            normalize_dependency_fingerprint(&hir_fingerprint),
        ];
        let admitted =
            admit_artifact(&input.artifact_json, &slots, library_roots).map_err(|error| {
                fail(
                    "CMP206",
                    InvalidRequest,
                    format!("proof admission refused authority: {error}"),
                )
            })?;
        let deps_hex = slots.iter().map(hex_slot).collect();
        live.push(LiveUse {
            operation,
            admitted,
            deps_bytes: slots.to_vec(),
            deps_hex,
        });
    }
    // Phase 3: attach with re-validation (obligation must be live in HIR/SSA).
    let mut hir = program.lower_to_ir().map_err(|error| {
        fail(
            "CMP204",
            InternalCompilerDefect,
            format!("validated semantic input failed HIR lowering: {error}"),
        )
    })?;
    for use_ in &live {
        hir.attach_proof_relationship(use_.admitted.relationship.clone(), &use_.deps_hex)
            .map_err(|mismatch| {
                fail(
                    "CMP206",
                    InvalidRequest,
                    format!("proof relationship refused by current HIR: {mismatch}"),
                )
            })?;
    }
    let mut ssa = program.lower_to_ssa_from_ir(&hir).map_err(|error| {
        fail(
            "CMP205",
            InternalCompilerDefect,
            format!("validated semantic input failed SSA lowering: {error}"),
        )
    })?;
    for use_ in &live {
        ssa.attach_proof_relationship(use_.admitted.relationship.clone(), &use_.deps_hex)
            .map_err(|mismatch| {
                fail(
                    "CMP206",
                    InvalidRequest,
                    format!("proof relationship refused by current SSA: {mismatch}"),
                )
            })?;
    }
    // Phase 4: authorize each use by fresh MNCS execution, then record.
    let mut diagnostics = Vec::new();
    for use_ in &live {
        let relationship = &use_.admitted.relationship;
        match authorize_reuse(
            &use_.admitted,
            &use_.admitted.artifact,
            &use_.deps_bytes,
            library_roots,
        ) {
            Ok(true) => {
                let outcome = ssa.record_proof_evidence(
                    &relationship.proof_identity,
                    &mncs_model::SemanticId(use_.operation.clone()),
                    &relationship.obligation,
                    &use_.deps_hex,
                    true,
                );
                if !outcome.recorded {
                    diagnostics.push(fail(
                        "CMP303",
                        mncs_model::CompilerDiagnosticKind::FailedTransformationRelation,
                        format!("proof evidence unexpectedly withheld: {}", outcome.reason),
                    ));
                }
            }
            Ok(false) => diagnostics.push(fail(
                "CMP303",
                mncs_model::CompilerDiagnosticKind::FailedTransformationRelation,
                format!(
                    "MNCS reuse check refused proof {} for this use; evidence withheld",
                    relationship.proof_identity
                ),
            )),
            Err(error) => {
                return Err(Box::new(fail(
                    "CMP206",
                    InvalidRequest,
                    format!("proof authorization failed: {error}"),
                )));
            }
        }
    }
    Ok(ProofLowering {
        hir,
        ssa,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mncs_model::{parse_proof_dep_corpus, DepCorroboration, DepVerdict};

    fn library_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../library")
    }

    fn corpus_artifact(case_id: &str, obligation: &str) -> (DepArtifact, String) {
        let path = format!(
            "{}/../../examples/execution/proof-dep-corpus.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).expect("proof-dep corpus");
        let cases = parse_proof_dep_corpus(&text).expect("parse proof-dep corpus");
        let case = cases
            .iter()
            .find(|case| case.id == case_id)
            .unwrap_or_else(|| panic!("corpus case {case_id}"));
        let artifact = DepArtifact::new(
            obligation,
            case.cells.clone(),
            case.count,
            case.first,
            case.second,
        );
        assert!(artifact.identity_is_valid(), "fresh artifact seals");
        let json = serde_json::to_string(&artifact).expect("serialize artifact");
        (artifact, json)
    }

    fn test_dependencies() -> Vec<[u8; 32]> {
        vec![
            normalize_dependency_fingerprint("mncs:obligation:test"),
            normalize_dependency_fingerprint("mncs:operation:test-add"),
            normalize_dependency_fingerprint("hir-content-test"),
        ]
    }

    #[test]
    fn cell_encoding_matches_checked_in_corpus() {
        // The admission transport must encode cells exactly as the pinned
        // corpus does; any drift fails here instead of silently forking the
        // MNCS-executed meaning from the differentially-checked one.
        let path = format!(
            "{}/../../examples/execution/proof-dep-corpus.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("corpus"))
                .expect("corpus JSON");
        let flagship = document
            .get("cases")
            .and_then(serde_json::Value::as_array)
            .expect("cases")
            .iter()
            .find(|case| {
                case.get("id").and_then(serde_json::Value::as_str)
                    == Some("flagship-plus-zero-right")
            })
            .expect("flagship case");
        let first = flagship
            .get("request")
            .and_then(|request| request.get("arguments"))
            .and_then(serde_json::Value::as_array)
            .and_then(|arguments| arguments.first())
            .and_then(|argument| argument.get("sequence"))
            .and_then(|sequence| sequence.get("values"))
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .expect("first flagship cell");
        let encoded =
            serde_json::to_value(dep_cell_value(&DepCell::new(DepTag::Nat, [0, 0, 0, 0])))
                .expect("encode cell");
        assert_eq!(&encoded, first);
    }

    #[test]
    fn admit_flagship_through_genuine_mncs_execution() {
        let (_artifact, json) = corpus_artifact(
            "flagship-plus-zero-right",
            "mncs:obligation:plus-zero-right",
        );
        let admitted = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect("flagship admits");
        assert_eq!(admitted.corroboration, DepCorroboration::AgreePass);
        assert!(admitted.relationship.consumable());
        assert_eq!(admitted.relationship.mncs_verdict, DepVerdict::Pass);
        assert!(admitted.relationship.assumptions.is_empty_set());
        assert!(admitted.seal_steps > 0);
        assert!(!admitted.program_fingerprint.is_empty());
        assert_eq!(admitted.relationship.dependencies.len(), 3);
    }

    #[test]
    fn admit_open_proof_records_canonical_set() {
        let (_artifact, json) =
            corpus_artifact("assumption-open-refl", "mncs:obligation:open-refl");
        let admitted =
            admit_artifact(&json, &test_dependencies(), &[library_root()]).expect("open admits");
        assert_eq!(admitted.corroboration, DepCorroboration::AgreePass);
        assert!(admitted.relationship.consumable());
        assert_eq!(admitted.relationship.assumptions.uses.len(), 1);
        assert_eq!(admitted.relationship.assumptions.uses[0].hyp, 1);
    }

    #[test]
    fn admit_refuses_fail_artifact() {
        let (_artifact, json) = corpus_artifact("undeclared-var", "mncs:obligation:undeclared");
        let error = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect_err("FAIL refuses");
        assert!(
            matches!(error, AdmissionError::ExecutionFailed { .. }),
            "{error}"
        );
    }

    #[test]
    fn admit_refuses_unknown_artifact() {
        let (_artifact, json) = corpus_artifact("unknown-opaque-proof", "mncs:obligation:opaque");
        let error = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect_err("UNKNOWN refuses");
        assert!(
            matches!(error, AdmissionError::ExecutionFailed { .. }),
            "{error}"
        );
    }

    #[test]
    fn admit_refuses_tampered_seal() {
        let (artifact, mut json) = corpus_artifact(
            "flagship-plus-zero-right",
            "mncs:obligation:plus-zero-right",
        );
        let marker = "\"identity\":\"mncs:proof-dep:";
        let start = json.find(marker).expect("identity") + marker.len();
        let flip = if json[start..].starts_with('a') {
            'b'
        } else {
            'a'
        };
        json.replace_range(start..start + 1, &flip.to_string());
        let error = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect_err("tamper refuses");
        assert_eq!(error, AdmissionError::StaleIdentity);
        let _ = artifact;
    }

    #[test]
    fn admit_refuses_wrong_kernel() {
        let (mut artifact, _) = corpus_artifact(
            "flagship-plus-zero-right",
            "mncs:obligation:plus-zero-right",
        );
        artifact.kernel = "mncs:proof-kernel:9.9".to_owned();
        artifact.reseal();
        assert!(artifact.identity_is_valid());
        let json = serde_json::to_string(&artifact).expect("serialize");
        let error = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect_err("kernel refuses");
        assert!(
            matches!(error, AdmissionError::WrongKernel { .. }),
            "{error}"
        );
    }

    #[test]
    fn admit_seals_obligation_string_without_judging_it() {
        // Admission seals whatever obligation string the artifact names
        // (even a 106-byte compiler obligation id); judging whether that
        // obligation is live happens at HIR/SSA attachment, not here.
        let path = format!(
            "{}/../../examples/execution/proof-dep-corpus.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).expect("corpus");
        let cases = parse_proof_dep_corpus(&text).expect("parse");
        let case = cases
            .iter()
            .find(|case| case.id == "assumption-open-refl")
            .expect("open");
        let long_obligation = "mncs:0.2:obligation:body:integer-overflow:9db4fac4b57262c2106e42f673805d891bb34555ca15f514dc63f31e373780a1";
        assert!(long_obligation.len() > 64);
        let artifact = DepArtifact::new(
            long_obligation,
            case.cells.clone(),
            case.count,
            case.first,
            case.second,
        );
        let json = serde_json::to_string(&artifact).expect("serialize");
        let admitted = admit_artifact(&json, &test_dependencies(), &[library_root()])
            .expect("long obligation admits");
        assert_eq!(admitted.relationship.obligation, long_obligation);
        assert!(admitted.relationship.consumable());
        // A different obligation string seals to a different relationship.
        let (_other, other_json) =
            corpus_artifact("assumption-open-refl", "mncs:obligation:open-refl");
        let other =
            admit_artifact(&other_json, &test_dependencies(), &[library_root()]).expect("admits");
        assert_ne!(
            admitted.relationship.proof_identity,
            other.relationship.proof_identity
        );
        assert_ne!(
            admitted.relationship.obligation,
            other.relationship.obligation
        );
    }

    #[test]
    fn authorize_reuse_accepts_current_use() {
        let (artifact, json) = corpus_artifact(
            "flagship-plus-zero-right",
            "mncs:obligation:plus-zero-right",
        );
        let roots = vec![library_root()];
        let admitted =
            admit_artifact(&json, &test_dependencies(), &roots).expect("flagship admits");
        let authorized = authorize_reuse(&admitted, &artifact, &test_dependencies(), &roots)
            .expect("reuse executes");
        assert!(authorized, "current use authorizes");
    }

    #[test]
    fn authorize_reuse_rejects_drifted_dependency() {
        let (artifact, json) = corpus_artifact("assumption-open-refl", "mncs:obligation:open-refl");
        let roots = vec![library_root()];
        let admitted = admit_artifact(&json, &test_dependencies(), &roots).expect("open admits");
        let mut drifted = test_dependencies();
        drifted[1] = normalize_dependency_fingerprint("mncs:operation:other-add");
        let authorized =
            authorize_reuse(&admitted, &artifact, &drifted, &roots).expect("reuse executes");
        assert!(!authorized, "drifted dependency withholds");
    }

    #[test]
    fn authorize_reuse_rejects_mutated_artifact() {
        let (mut artifact, json) =
            corpus_artifact("assumption-open-refl", "mncs:obligation:open-refl");
        let roots = vec![library_root()];
        let admitted = admit_artifact(&json, &test_dependencies(), &roots).expect("open admits");
        // Mutating without resealing breaks the seal: stale, refused loudly.
        artifact.cells[2] = mncs_model::DepCellSer::of(&DepCell::new(DepTag::Nat, [0, 0, 0, 0]));
        let stale = authorize_reuse(&admitted, &artifact, &test_dependencies(), &roots)
            .expect_err("mutated cells refuse");
        assert_eq!(stale, AdmissionError::StaleIdentity);
        // Mutating AND resealing mints a different artifact identity, which
        // the relationship admitted for the original does not cover.
        artifact.reseal();
        assert!(artifact.identity_is_valid());
        let foreign = authorize_reuse(&admitted, &artifact, &test_dependencies(), &roots)
            .expect_err("re-identified artifact refuses");
        assert!(
            matches!(foreign, AdmissionError::IdentityEchoMismatch(_)),
            "{foreign}"
        );
    }
}
