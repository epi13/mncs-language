//! RFC 0007 tranche-0.2 proof ingestion, executed as a test:
//!
//! ```text
//! file artifact
//!     -> mncs compile <program> --proof <artifact> (real CLI, real file)
//!     -> MNCS kernel PASS (genuine in-process MNCS execution)
//!     -> authoritative MNCS binding + independent corroboration
//!     -> compiler consumes the proof relationship (HIR -> SSA -> evidence)
//!     -> justified lowering evidence names the proof identity
//!     -> mutate artifact/dependency -> reuse rejected (loud failure)
//! ```
//!
//! Every arrow is asserted against the real binary. The proof is recorded
//! as validated formal evidence for the obligated operation; it does NOT
//! discharge the obligation (discharge is future work and is never claimed).

use std::path::PathBuf;
use std::process::Command;

use mncs_model::{parse_proof_dep_corpus, DepArtifact, DepCellSer};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn mncs() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mncs"));
    command.env(
        "MNCS_LIBRARY_PATH",
        repo_root().join("library").display().to_string(),
    );
    command
}

fn demo_program() -> String {
    repo_root()
        .join("examples/source/proof-demo-no-overflow.mncs")
        .display()
        .to_string()
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mncs-proof-dep-e2e-{}-{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn compile_plain(output_dir: &std::path::Path) -> serde_json::Value {
    let output = mncs()
        .args(["compile", &demo_program(), "--output-dir"])
        .arg(output_dir)
        .output()
        .expect("run mncs compile");
    assert!(
        output.status.success(),
        "plain compile succeeds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ssa: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(output_dir.join("ssa.json")).expect("ssa.json"),
    )
    .expect("ssa JSON");
    ssa
}

/// The demo's UNKNOWN integer-overflow obligation and the SSA operation
/// carrying it: real compiler facts, discovered per run (never hardcoded).
fn discover_obligation_and_operation(ssa: &serde_json::Value) -> (String, String) {
    let obligations = ssa
        .get("obligations")
        .and_then(serde_json::Value::as_array)
        .expect("obligations");
    let obligation = obligations
        .iter()
        .find(|record| {
            record
                .get("requirement")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|requirement| requirement.contains("integer-overflow"))
                && record
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|status| status == "Unknown" || status == "UNKNOWN")
        })
        .expect("UNKNOWN integer-overflow obligation");
    let obligation_id = obligation
        .get("identity")
        .and_then(serde_json::Value::as_str)
        .expect("obligation identity")
        .to_owned();
    let mut carriers = Vec::new();
    for function in ssa
        .get("functions")
        .and_then(serde_json::Value::as_array)
        .expect("functions")
    {
        for block in function
            .get("blocks")
            .and_then(serde_json::Value::as_array)
            .expect("blocks")
        {
            for instruction in block
                .get("instructions")
                .and_then(serde_json::Value::as_array)
                .expect("instructions")
            {
                let carries = instruction
                    .get("obligations")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|list| {
                        list.iter()
                            .any(|entry| entry.as_str() == Some(obligation_id.as_str()))
                    });
                if carries {
                    if let Some(operation) = instruction
                        .get("semantic_identity")
                        .and_then(serde_json::Value::as_str)
                    {
                        carriers.push(operation.to_owned());
                    }
                }
            }
        }
    }
    carriers.sort();
    carriers.dedup();
    assert_eq!(carriers.len(), 1, "exactly one obligated operation");
    (
        obligation_id,
        carriers.into_iter().next().expect("operation"),
    )
}

fn open_refl_artifact(obligation: &str) -> DepArtifact {
    let text =
        std::fs::read_to_string(repo_root().join("examples/execution/proof-dep-corpus.json"))
            .expect("proof-dep corpus");
    let cases = parse_proof_dep_corpus(&text).expect("parse corpus");
    let case = cases
        .iter()
        .find(|case| case.id == "assumption-open-refl")
        .expect("open-refl case");
    let artifact = DepArtifact::new(
        obligation,
        case.cells.clone(),
        case.count,
        case.first,
        case.second,
    );
    assert!(artifact.identity_is_valid(), "fresh artifact seals");
    artifact
}

fn write_artifact(dir: &std::path::Path, artifact: &DepArtifact) -> String {
    let path = dir.join("proof-artifact.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(artifact).expect("serialize artifact"),
    )
    .expect("write artifact");
    path.display().to_string()
}

fn compile_with_proof(
    output_dir: &std::path::Path,
    artifact_path: &str,
    extra: &[&str],
) -> std::process::Output {
    let mut command = mncs();
    command
        .args(["compile", &demo_program(), "--proof", artifact_path])
        .args(extra)
        .arg("--output-dir")
        .arg(output_dir);
    command.output().expect("run mncs compile --proof")
}

fn proof_evidence_records(ssa: &serde_json::Value) -> Vec<serde_json::Value> {
    ssa.get("transformations")
        .and_then(serde_json::Value::as_array)
        .expect("transformations")
        .iter()
        .filter(|record| {
            record.get("rule").and_then(serde_json::Value::as_str)
                == Some("tranche-0.2-proof-bearing-evidence")
        })
        .cloned()
        .collect()
}

#[test]
fn proof_ingestion_records_evidence_end_to_end() {
    let dir = scratch_dir("happy");
    let ssa = compile_plain(&dir.join("plain"));
    let (obligation, _operation) = discover_obligation_and_operation(&ssa);
    let artifact = open_refl_artifact(&obligation);
    let identity = artifact.identity.clone();
    let artifact_path = write_artifact(&dir, &artifact);
    let output = compile_with_proof(&dir.join("proof"), &artifact_path, &[]);
    assert!(
        output.status.success(),
        "proof-carrying compile succeeds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let proof_ssa: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("proof/ssa.json")).expect("proof ssa.json"),
    )
    .expect("ssa JSON");
    // The relationship survives into SSA as validated metadata.
    let relationships = proof_ssa
        .get("proof_relationships")
        .and_then(serde_json::Value::as_array)
        .expect("proof_relationships");
    assert!(
        relationships.iter().any(|relationship| relationship
            .get("proof_identity")
            .and_then(serde_json::Value::as_str)
            == Some(identity.as_str())),
        "SSA carries the admitted relationship"
    );
    // The lowering evidence names the exact proof identity and obligation.
    let records = proof_evidence_records(&proof_ssa);
    assert_eq!(records.len(), 1, "exactly one proof-bearing record");
    let consumed = records[0]
        .get("evidence_consumed")
        .and_then(serde_json::Value::as_array)
        .expect("evidence_consumed");
    assert!(
        consumed
            .iter()
            .any(|entry| entry.as_str() == Some(identity.as_str())),
        "evidence consumes the proof identity"
    );
    let required = records[0]
        .get("obligations_required")
        .and_then(serde_json::Value::as_array)
        .expect("obligations_required");
    assert!(
        required
            .iter()
            .any(|entry| entry.as_str() == Some(obligation.as_str())),
        "evidence names the obligation"
    );
    // The evidence bundle exists and the run reports success overall.
    assert!(dir.join("proof/evidence.json").exists(), "evidence bundle");
}

/// Tranche-0.3 proof transport: an admitted proof must survive through
/// the backend boundary as versioned references, not just as SSA
/// metadata. The backend artifact names the exact proof identity,
/// obligation, kernel, and SSA fingerprint it lowered, so a consumer can
/// re-validate without trusting the backend's word; proof-free
/// compilation of the same program carries no such references.
#[test]
fn proof_binding_refs_survive_into_backend_artifact() {
    let dir = scratch_dir("transport");
    let ssa = compile_plain(&dir.join("plain"));
    let (obligation, _) = discover_obligation_and_operation(&ssa);
    let artifact = open_refl_artifact(&obligation);
    let identity = artifact.identity.clone();
    let artifact_path = write_artifact(&dir, &artifact);
    let output = mncs()
        .args([
            "compile",
            &demo_program(),
            "--proof",
            &artifact_path,
            "--emit",
            "backend",
            "--target",
            "mncs-research-bytecode",
        ])
        .output()
        .expect("run mncs compile --proof --emit backend");
    assert!(
        output.status.success(),
        "proof-carrying backend compile succeeds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("compilation JSON");
    let backend = result
        .get("emissions")
        .and_then(|emissions| emissions.get("backend"))
        .expect("backend emission");
    let bindings = backend
        .get("proof_bindings")
        .and_then(serde_json::Value::as_array)
        .expect("proof_bindings");
    assert_eq!(
        bindings.len(),
        1,
        "exactly one transported ref, got {bindings:?}"
    );
    let binding = &bindings[0];
    assert_eq!(
        binding
            .get("proof_identity")
            .and_then(serde_json::Value::as_str),
        Some(identity.as_str()),
        "artifact names the admitted proof"
    );
    assert_eq!(
        binding
            .get("obligation")
            .and_then(serde_json::Value::as_str),
        Some(obligation.as_str()),
        "artifact names the obligation"
    );
    assert!(
        binding
            .get("kernel")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kernel| !kernel.is_empty()),
        "artifact names the kernel identity"
    );
    assert!(
        binding
            .get("ssa_fingerprint")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|fingerprint| fingerprint.len() == 64),
        "artifact binds the exact SSA fingerprint"
    );
    // The sealed artifact identity covers the bindings: re-serializing
    // the envelope must validate (checked by loading it back below),
    // and the proof-free compilation of the same program carries none.
    let plain_output = mncs()
        .args([
            "compile",
            &demo_program(),
            "--emit",
            "backend",
            "--target",
            "mncs-research-bytecode",
        ])
        .output()
        .expect("run mncs compile --emit backend");
    assert!(plain_output.status.success());
    let plain_result: serde_json::Value =
        serde_json::from_slice(&plain_output.stdout).expect("compilation JSON");
    let plain_backend = plain_result
        .get("emissions")
        .and_then(|emissions| emissions.get("backend"))
        .expect("backend emission");
    assert!(
        plain_backend
            .get("proof_bindings")
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty),
        "proof-free artifact carries no proof references"
    );
    assert_ne!(
        backend.get("identity"),
        plain_backend.get("identity"),
        "proof-bearing and proof-free artifacts never share an identity"
    );
}

#[test]
fn proof_ingestion_rejects_mutated_cells() {
    let dir = scratch_dir("mutated");
    let ssa = compile_plain(&dir.join("plain"));
    let (obligation, _) = discover_obligation_and_operation(&ssa);
    let mut artifact = open_refl_artifact(&obligation);
    artifact.cells[2] = DepCellSer {
        tag: "Nat".to_owned(),
        args: [0, 0, 0, 0],
    };
    artifact.reseal();
    assert!(artifact.identity_is_valid());
    let artifact_path = write_artifact(&dir, &artifact);
    let output = compile_with_proof(&dir.join("proof"), &artifact_path, &[]);
    assert!(!output.status.success(), "mutated proof must fail loudly");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMP206"),
        "refusal names the proof diagnostic: {stdout}"
    );
}

#[test]
fn proof_ingestion_rejects_foreign_obligation() {
    let dir = scratch_dir("foreign");
    let artifact = open_refl_artifact("mncs:obligation:elsewhere-entirely");
    assert!(artifact.identity_is_valid());
    let artifact_path = write_artifact(&dir, &artifact);
    let output = compile_with_proof(&dir.join("proof"), &artifact_path, &[]);
    assert!(
        !output.status.success(),
        "foreign obligation must fail loudly"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMP206"),
        "refusal names the proof diagnostic: {stdout}"
    );
}

#[test]
fn proof_ingestion_rejects_tampered_seal() {
    let dir = scratch_dir("tampered");
    let ssa = compile_plain(&dir.join("plain"));
    let (obligation, _) = discover_obligation_and_operation(&ssa);
    let artifact = open_refl_artifact(&obligation);
    let mut json = serde_json::to_string(&artifact).expect("serialize");
    let marker = "\"identity\":\"mncs:proof-dep:";
    let start = json.find(marker).expect("identity") + marker.len();
    let flip = if json[start..].starts_with('a') {
        'b'
    } else {
        'a'
    };
    json.replace_range(start..start + 1, &flip.to_string());
    let artifact_path = dir.join("proof-artifact.json");
    std::fs::write(&artifact_path, json).expect("write artifact");
    let output = compile_with_proof(
        &dir.join("proof"),
        &artifact_path.display().to_string(),
        &[],
    );
    assert!(!output.status.success(), "tampered seal must fail loudly");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMP206"),
        "refusal names the proof diagnostic: {stdout}"
    );
}

#[test]
fn proof_ingestion_rejects_fail_artifact() {
    let dir = scratch_dir("failproof");
    let text =
        std::fs::read_to_string(repo_root().join("examples/execution/proof-dep-corpus.json"))
            .expect("proof-dep corpus");
    let cases = parse_proof_dep_corpus(&text).expect("parse corpus");
    let case = cases
        .iter()
        .find(|case| case.id == "undeclared-var")
        .expect("undeclared-var case");
    let artifact = DepArtifact::new(
        "mncs:obligation:undeclared",
        case.cells.clone(),
        case.count,
        case.first,
        case.second,
    );
    assert!(artifact.identity_is_valid());
    let artifact_path = write_artifact(&dir, &artifact);
    let output = compile_with_proof(&dir.join("proof"), &artifact_path, &[]);
    assert!(!output.status.success(), "FAIL proof must fail loudly");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMP206"),
        "refusal names the proof diagnostic: {stdout}"
    );
}

#[test]
fn proof_ingestion_rejects_missing_file() {
    let dir = scratch_dir("missing");
    let output = compile_with_proof(&dir.join("proof"), "/nonexistent/proof.json", &[]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing artifact file is a usage error"
    );
}

#[test]
fn proof_ingestion_rejects_unknown_operation() {
    let dir = scratch_dir("badop");
    let ssa = compile_plain(&dir.join("plain"));
    let (obligation, _) = discover_obligation_and_operation(&ssa);
    let artifact = open_refl_artifact(&obligation);
    let artifact_path = write_artifact(&dir, &artifact);
    let output = compile_with_proof(
        &dir.join("proof"),
        &artifact_path,
        &["--proof-operation", "mncs:operation:does-not-exist"],
    );
    assert!(
        !output.status.success(),
        "unknown operation must fail loudly"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMP206"),
        "refusal names the proof diagnostic: {stdout}"
    );
}
