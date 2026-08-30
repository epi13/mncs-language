//! Process-local structural cost accounting for bounded MNCS experiments.
//!
//! The counters are observability only. They do not participate in semantic
//! decisions and are intentionally reset at an explicit CLI experiment
//! boundary. Reuse remains governed by artifact identities and receipts.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const COST_REPORT_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostReport {
    pub schema_version: String,
    pub source_parse_count: u64,
    pub semantic_validation_count: u64,
    pub hir_build_count: u64,
    pub ssa_build_count: u64,
    pub backend_compile_count: u64,
    pub backend_validation_count: u64,
    pub backend_session_count: u64,
    pub artifact_decode_count: u64,
    pub artifact_hash_count: u64,
    pub canonical_hash_count: u64,
    pub serialization_count: u64,
    pub corpus_case_count: u64,
    pub stateful_trace_count: u64,
    pub stateful_call_count: u64,
    pub reused_stage_count: u64,
    pub reused_execution_count: u64,
    pub prefix_checkpoint_count: u64,
    pub prefix_reuse_count: u64,
    pub new_execution_count: u64,
    pub stage_invocations: BTreeMap<String, u64>,
    pub wall_time_by_stage_ns: BTreeMap<String, u128>,
}

impl CostReport {
    fn fresh() -> Self {
        Self {
            schema_version: COST_REPORT_SCHEMA_VERSION.to_owned(),
            ..Self::default()
        }
    }
}

static COST_REPORT: OnceLock<Mutex<CostReport>> = OnceLock::new();

fn report() -> &'static Mutex<CostReport> {
    COST_REPORT.get_or_init(|| Mutex::new(CostReport::fresh()))
}

pub fn reset_cost_report() {
    *report().lock().expect("cost report mutex poisoned") = CostReport::fresh();
}

pub fn cost_report() -> CostReport {
    report().lock().expect("cost report mutex poisoned").clone()
}

pub fn record_counter(counter: &'static str) {
    let mut report = report().lock().expect("cost report mutex poisoned");
    match counter {
        "source_parse" => report.source_parse_count += 1,
        "semantic_validation" => report.semantic_validation_count += 1,
        "hir_build" => report.hir_build_count += 1,
        "ssa_build" => report.ssa_build_count += 1,
        "backend_compile" => report.backend_compile_count += 1,
        "backend_validation" => report.backend_validation_count += 1,
        "backend_session" => report.backend_session_count += 1,
        "artifact_decode" => report.artifact_decode_count += 1,
        "artifact_hash" => report.artifact_hash_count += 1,
        "canonical_hash" => report.canonical_hash_count += 1,
        "serialization" => report.serialization_count += 1,
        "corpus_case" => report.corpus_case_count += 1,
        "stateful_trace" => report.stateful_trace_count += 1,
        "stateful_call" => report.stateful_call_count += 1,
        "reused_stage" => report.reused_stage_count += 1,
        "reused_execution" => report.reused_execution_count += 1,
        "prefix_checkpoint" => report.prefix_checkpoint_count += 1,
        "prefix_reuse" => report.prefix_reuse_count += 1,
        "new_execution" => report.new_execution_count += 1,
        _ => {}
    }
}

pub fn record_stage(stage: &str, elapsed: Duration) {
    let mut report = report().lock().expect("cost report mutex poisoned");
    *report
        .stage_invocations
        .entry(stage.to_owned())
        .or_default() += 1;
    *report
        .wall_time_by_stage_ns
        .entry(stage.to_owned())
        .or_default() += elapsed.as_nanos();
}
