use mncs_compiler::{elaborate_program_with_resolver, ModuleResolver};
use mncs_conformance::{discover_contracts, generate_cases, run_conformance, ConformanceOptions};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

#[derive(Default)]
struct MapResolver {
    modules: std::collections::BTreeMap<String, String>,
}

impl MapResolver {
    fn envelope(&self, name: &str, text: String) -> SourceEnvelope {
        SourceEnvelope::inline(SourceArtifactKind::Program, name.to_owned(), text)
    }
}

impl ModuleResolver for MapResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        self.modules
            .get(module)
            .cloned()
            .map(|text| self.envelope(module, text))
    }
}

fn elaborate(text: &str) -> mncs_model::Program {
    let resolver = MapResolver::default();
    let envelope = resolver.envelope("root", text.to_owned());
    let parsed = parse(&envelope);
    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.expect("ast");
    elaborate_program_with_resolver(&ast, &resolver)
        .0
        .expect("elaborates")
}

const ARITHMETIC: &str = r#"
mncs 0.9;
module test.arith;
fn add(a: i64, b: i64) -> (result: i64)
    property prop_roundtrip
    metamorphic prop_commutes
{
    return a +% b;
}
fn sub(a: i64, b: i64) -> (result: i64) {
    return a -% b;
}
fn prop_roundtrip(x: i64, y: i64) -> (result: bool) {
    return sub(add(x, y), y) == x;
}
fn prop_commutes(x: i64, y: i64) -> (result: bool) {
    return add(x, y) == add(y, x);
}
"#;

const MUTANT: &str = r#"
mncs 0.9;
module test.mutant;
fn add(a: i64, b: i64) -> (result: i64)
    property prop_roundtrip
{
    return (a +% b) +% 1;
}
fn sub(a: i64, b: i64) -> (result: i64) {
    return a -% b;
}
fn prop_roundtrip(x: i64, y: i64) -> (result: bool) {
    return sub(add(x, y), y) == x;
}
"#;

/// Regression test for the executor identity-cache blowup: every nested
/// MNCS call used to re-serialize and re-hash the whole SSA module, so
/// call-heavy predicates (the vision observer) could never complete. Two
/// hundred sequential nested calls must cost a handful of canonical hashes — one
/// slow fingerprint per top-level execution — not hundreds.
const CALL_CHAIN: &str = r#"
mncs 0.9;
module test.chain;
fn bump(x: i64) -> (result: i64) {
    return x +% 1;
}
fn op(x: i64) -> (result: i64)
    property prop_chain
{
    return x;
}
fn prop_chain(x: i64) -> (result: bool) {
    let a000: i64 = bump(x);
    let a001: i64 = bump(a000);
    let a002: i64 = bump(a001);
    let a003: i64 = bump(a002);
    let a004: i64 = bump(a003);
    let a005: i64 = bump(a004);
    let a006: i64 = bump(a005);
    let a007: i64 = bump(a006);
    let a008: i64 = bump(a007);
    let a009: i64 = bump(a008);
    let a010: i64 = bump(a009);
    let a011: i64 = bump(a010);
    let a012: i64 = bump(a011);
    let a013: i64 = bump(a012);
    let a014: i64 = bump(a013);
    let a015: i64 = bump(a014);
    let a016: i64 = bump(a015);
    let a017: i64 = bump(a016);
    let a018: i64 = bump(a017);
    let a019: i64 = bump(a018);
    let a020: i64 = bump(a019);
    let a021: i64 = bump(a020);
    let a022: i64 = bump(a021);
    let a023: i64 = bump(a022);
    let a024: i64 = bump(a023);
    let a025: i64 = bump(a024);
    let a026: i64 = bump(a025);
    let a027: i64 = bump(a026);
    let a028: i64 = bump(a027);
    let a029: i64 = bump(a028);
    let a030: i64 = bump(a029);
    let a031: i64 = bump(a030);
    let a032: i64 = bump(a031);
    let a033: i64 = bump(a032);
    let a034: i64 = bump(a033);
    let a035: i64 = bump(a034);
    let a036: i64 = bump(a035);
    let a037: i64 = bump(a036);
    let a038: i64 = bump(a037);
    let a039: i64 = bump(a038);
    let a040: i64 = bump(a039);
    let a041: i64 = bump(a040);
    let a042: i64 = bump(a041);
    let a043: i64 = bump(a042);
    let a044: i64 = bump(a043);
    let a045: i64 = bump(a044);
    let a046: i64 = bump(a045);
    let a047: i64 = bump(a046);
    let a048: i64 = bump(a047);
    let a049: i64 = bump(a048);
    let a050: i64 = bump(a049);
    let a051: i64 = bump(a050);
    let a052: i64 = bump(a051);
    let a053: i64 = bump(a052);
    let a054: i64 = bump(a053);
    let a055: i64 = bump(a054);
    let a056: i64 = bump(a055);
    let a057: i64 = bump(a056);
    let a058: i64 = bump(a057);
    let a059: i64 = bump(a058);
    let a060: i64 = bump(a059);
    let a061: i64 = bump(a060);
    let a062: i64 = bump(a061);
    let a063: i64 = bump(a062);
    let a064: i64 = bump(a063);
    let a065: i64 = bump(a064);
    let a066: i64 = bump(a065);
    let a067: i64 = bump(a066);
    let a068: i64 = bump(a067);
    let a069: i64 = bump(a068);
    let a070: i64 = bump(a069);
    let a071: i64 = bump(a070);
    let a072: i64 = bump(a071);
    let a073: i64 = bump(a072);
    let a074: i64 = bump(a073);
    let a075: i64 = bump(a074);
    let a076: i64 = bump(a075);
    let a077: i64 = bump(a076);
    let a078: i64 = bump(a077);
    let a079: i64 = bump(a078);
    let a080: i64 = bump(a079);
    let a081: i64 = bump(a080);
    let a082: i64 = bump(a081);
    let a083: i64 = bump(a082);
    let a084: i64 = bump(a083);
    let a085: i64 = bump(a084);
    let a086: i64 = bump(a085);
    let a087: i64 = bump(a086);
    let a088: i64 = bump(a087);
    let a089: i64 = bump(a088);
    let a090: i64 = bump(a089);
    let a091: i64 = bump(a090);
    let a092: i64 = bump(a091);
    let a093: i64 = bump(a092);
    let a094: i64 = bump(a093);
    let a095: i64 = bump(a094);
    let a096: i64 = bump(a095);
    let a097: i64 = bump(a096);
    let a098: i64 = bump(a097);
    let a099: i64 = bump(a098);
    let a100: i64 = bump(a099);
    let a101: i64 = bump(a100);
    let a102: i64 = bump(a101);
    let a103: i64 = bump(a102);
    let a104: i64 = bump(a103);
    let a105: i64 = bump(a104);
    let a106: i64 = bump(a105);
    let a107: i64 = bump(a106);
    let a108: i64 = bump(a107);
    let a109: i64 = bump(a108);
    let a110: i64 = bump(a109);
    let a111: i64 = bump(a110);
    let a112: i64 = bump(a111);
    let a113: i64 = bump(a112);
    let a114: i64 = bump(a113);
    let a115: i64 = bump(a114);
    let a116: i64 = bump(a115);
    let a117: i64 = bump(a116);
    let a118: i64 = bump(a117);
    let a119: i64 = bump(a118);
    let a120: i64 = bump(a119);
    let a121: i64 = bump(a120);
    let a122: i64 = bump(a121);
    let a123: i64 = bump(a122);
    let a124: i64 = bump(a123);
    let a125: i64 = bump(a124);
    let a126: i64 = bump(a125);
    let a127: i64 = bump(a126);
    let a128: i64 = bump(a127);
    let a129: i64 = bump(a128);
    let a130: i64 = bump(a129);
    let a131: i64 = bump(a130);
    let a132: i64 = bump(a131);
    let a133: i64 = bump(a132);
    let a134: i64 = bump(a133);
    let a135: i64 = bump(a134);
    let a136: i64 = bump(a135);
    let a137: i64 = bump(a136);
    let a138: i64 = bump(a137);
    let a139: i64 = bump(a138);
    let a140: i64 = bump(a139);
    let a141: i64 = bump(a140);
    let a142: i64 = bump(a141);
    let a143: i64 = bump(a142);
    let a144: i64 = bump(a143);
    let a145: i64 = bump(a144);
    let a146: i64 = bump(a145);
    let a147: i64 = bump(a146);
    let a148: i64 = bump(a147);
    let a149: i64 = bump(a148);
    let a150: i64 = bump(a149);
    let a151: i64 = bump(a150);
    let a152: i64 = bump(a151);
    let a153: i64 = bump(a152);
    let a154: i64 = bump(a153);
    let a155: i64 = bump(a154);
    let a156: i64 = bump(a155);
    let a157: i64 = bump(a156);
    let a158: i64 = bump(a157);
    let a159: i64 = bump(a158);
    let a160: i64 = bump(a159);
    let a161: i64 = bump(a160);
    let a162: i64 = bump(a161);
    let a163: i64 = bump(a162);
    let a164: i64 = bump(a163);
    let a165: i64 = bump(a164);
    let a166: i64 = bump(a165);
    let a167: i64 = bump(a166);
    let a168: i64 = bump(a167);
    let a169: i64 = bump(a168);
    let a170: i64 = bump(a169);
    let a171: i64 = bump(a170);
    let a172: i64 = bump(a171);
    let a173: i64 = bump(a172);
    let a174: i64 = bump(a173);
    let a175: i64 = bump(a174);
    let a176: i64 = bump(a175);
    let a177: i64 = bump(a176);
    let a178: i64 = bump(a177);
    let a179: i64 = bump(a178);
    let a180: i64 = bump(a179);
    let a181: i64 = bump(a180);
    let a182: i64 = bump(a181);
    let a183: i64 = bump(a182);
    let a184: i64 = bump(a183);
    let a185: i64 = bump(a184);
    let a186: i64 = bump(a185);
    let a187: i64 = bump(a186);
    let a188: i64 = bump(a187);
    let a189: i64 = bump(a188);
    let a190: i64 = bump(a189);
    let a191: i64 = bump(a190);
    let a192: i64 = bump(a191);
    let a193: i64 = bump(a192);
    let a194: i64 = bump(a193);
    let a195: i64 = bump(a194);
    let a196: i64 = bump(a195);
    let a197: i64 = bump(a196);
    let a198: i64 = bump(a197);
    let a199: i64 = bump(a198);
    return a199 == a198 +% 1;
}
"#;

#[test]
fn nested_calls_reuse_execution_identity() {
    use mncs_model::{
        ExecutionRequest, ExecutionStatus, ExecutionTarget, ExecutionValue, IntegerType,
        EXECUTION_REQUEST_SCHEMA_VERSION,
    };
    let program = elaborate(CALL_CHAIN);
    let ssa = program.lower_to_ssa().expect("lowers");
    mncs_model::reset_cost_report();
    let result = mncs_model::execute_ssa_module(
        &program,
        &ssa,
        &ExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: program.module.clone(),
                function: "prop_chain".to_owned(),
            },
            arguments: vec![ExecutionValue::Integer {
                value: 3,
                ty: IntegerType {
                    bits: 64,
                    signed: true,
                },
            }],
            step_budget: 16384,
            policy: Default::default(),
        },
    );
    assert_eq!(result.status, ExecutionStatus::Returned);
    assert_eq!(
        result.returned,
        vec![ExecutionValue::Boolean { value: true }]
    );
    let report_counts = mncs_model::cost_report();
    assert!(
        report_counts.canonical_hash_count <= 60,
        "nested calls re-fingerprinted the module: {} canonical hashes ({} serializations)",
        report_counts.canonical_hash_count,
        report_counts.serialization_count,
    );
}

fn reference_only(cases: usize) -> ConformanceOptions {
    ConformanceOptions {
        seed: 42,
        cases_per_predicate: cases,
        backends: Vec::new(),
        step_budget: 4096,
        only_predicates: Vec::new(),
    }
}

#[test]
fn discovery_finds_executable_bindings() {
    let program = elaborate(ARITHMETIC);
    let contracts = discover_contracts(&program);
    assert_eq!(contracts.len(), 2);
    assert!(contracts.iter().all(|contract| contract.operation == "add"));
    let kinds: Vec<&str> = contracts
        .iter()
        .map(|contract| contract.kind.as_str())
        .collect();
    assert!(kinds.contains(&"property") && kinds.contains(&"metamorphic"));
    for contract in &contracts {
        assert!(!contract.predicate_identity.0.is_empty());
        assert!(contract.predicate_identity.0.starts_with("mncs:"));
    }
}

#[test]
fn generation_is_deterministic_per_seed() {
    let program = elaborate(ARITHMETIC);
    let predicate = program
        .functions
        .iter()
        .find(|function| function.name == "prop_roundtrip")
        .unwrap();
    let first = generate_cases(&predicate.inputs, 7, 10).expect("generates");
    let second = generate_cases(&predicate.inputs, 7, 10).expect("generates");
    assert_eq!(first, second);
    assert_eq!(first.len(), 10);
    // Small counts are boundary-dominated (identical across seeds by design);
    // past the boundary budget the seeded stream engages and must diverge.
    let streamed = generate_cases(&predicate.inputs, 7, 20).expect("generates");
    assert_eq!(streamed.len(), 20);
    let other = generate_cases(&predicate.inputs, 8, 20).expect("generates");
    assert_ne!(streamed, other, "different seeds diverge");
    // Boundary values lead: i64 corners of the first cases.
    assert!(first.iter().any(|case| case.iter().any(|value| matches!(
        value,
        mncs_model::ExecutionValue::Integer { value: v, .. } if *v == i64::MIN as i128 || *v == i64::MAX as i128
    ))));
}

#[test]
fn nullary_predicates_yield_one_empty_case() {
    let program = elaborate(
        r#"
mncs 0.9;
module test.nullary;
fn tick() -> (result: i64)
    property law_tick
{
    return 1;
}
fn law_tick() -> (result: bool) {
    return tick() == 1;
}
"#,
    );
    let predicate = program
        .functions
        .iter()
        .find(|function| function.name == "law_tick")
        .unwrap();
    let cases = generate_cases(&predicate.inputs, 1, 8).expect("generates");
    assert_eq!(cases, vec![Vec::new()]);
}

#[test]
fn structured_parameters_are_unsupported_not_silent() {
    let program = elaborate(
        r#"
mncs 0.9;
module test.structured;
fn first(xs: [i64; 4]) -> (result: i64)
    property law_first
{
    return xs[0];
}
fn law_first(xs: [i64; 4]) -> (result: bool) {
    return first(xs) == xs[0];
}
"#,
    );
    let report = run_conformance(&program, &reference_only(4));
    assert_eq!(report.predicates.len(), 1);
    assert_eq!(report.predicates[0].status, "unsupported");
    assert_eq!(report.summary.unsupported, 1);
    assert_eq!(report.summary.fail, 0);
}

#[test]
fn correct_implementation_passes_reference() {
    let program = elaborate(ARITHMETIC);
    let report = run_conformance(&program, &reference_only(8));
    assert_eq!(report.summary.fail, 0, "{:?}", report.summary);
    assert!(report.summary.pass > 0);
    assert!(report
        .predicates
        .iter()
        .all(|predicate| predicate.status == "tested"));
    for predicate in &report.predicates {
        for case in &predicate.cases {
            assert!(case.determinism_stable, "{}", case.id);
            assert_eq!(case.reference.verdict, "pass");
        }
    }
    assert!(!report.failed());
    assert!(report.subject_fingerprint.is_some());
}

#[test]
fn mutant_implementation_is_rejected() {
    let program = elaborate(MUTANT);
    let report = run_conformance(&program, &reference_only(8));
    assert!(report.failed());
    assert!(report.summary.fail > 0);
    assert_eq!(report.predicates.len(), 1);
    // Every generated case observes the off-by-one: sub(add(x,y),y) == x+1.
    assert!(report.predicates[0]
        .cases
        .iter()
        .all(|case| case.reference.verdict == "fail"));
}

#[test]
fn unknown_backends_are_unknown_never_pass() {
    let program = elaborate(ARITHMETIC);
    let report = run_conformance(
        &program,
        &ConformanceOptions {
            seed: 1,
            cases_per_predicate: 2,
            backends: vec!["mncs-no-such-backend".to_owned()],
            step_budget: 4096,
            only_predicates: Vec::new(),
        },
    );
    assert_eq!(report.summary.fail, 0);
    assert!(report.summary.unknown > 0);
    for predicate in &report.predicates {
        for case in &predicate.cases {
            assert_eq!(case.backends.len(), 1);
            assert_eq!(case.backends[0].verdict, "unknown");
            assert_eq!(case.backends[0].status, "not_lowered");
        }
    }
}

#[test]
fn evidence_attachment_discharges_contract_obligations() {
    use mncs_conformance::attach_evidence;
    let mut program = elaborate(ARITHMETIC);
    let before: Vec<String> = program
        .generate_obligations()
        .obligations
        .iter()
        .filter(|obligation| obligation.method == "language-evidence-binding")
        .map(|obligation| format!("{:?}", obligation.status))
        .collect();
    assert_eq!(before.len(), 2);
    assert!(
        before.iter().all(|status| status == "Unknown"),
        "{before:?}"
    );
    let report = run_conformance(&program, &reference_only(4));
    let attached = attach_evidence(&mut program, &report);
    assert_eq!(attached, 2);
    let after: Vec<String> = program
        .generate_obligations()
        .obligations
        .iter()
        .filter(|obligation| obligation.method == "language-evidence-binding")
        .map(|obligation| format!("{:?}", obligation.status))
        .collect();
    assert!(after.iter().all(|status| status == "Pass"), "{after:?}");
    // Reports are deterministic: the same run shares one content identity,
    // and re-attaching it adds nothing.
    let report_again = run_conformance(&program, &reference_only(4));
    assert_eq!(report.report_sha256(), report_again.report_sha256());
    assert_eq!(attach_evidence(&mut program, &report_again), 0);
}

#[test]
fn mutant_evidence_is_never_attached() {
    use mncs_conformance::attach_evidence;
    let mut program = elaborate(MUTANT);
    let report = run_conformance(&program, &reference_only(4));
    assert_eq!(attach_evidence(&mut program, &report), 0);
    let statuses: Vec<String> = program
        .generate_obligations()
        .obligations
        .iter()
        .filter(|obligation| obligation.method == "language-evidence-binding")
        .map(|obligation| format!("{:?}", obligation.status))
        .collect();
    assert!(
        statuses.iter().all(|status| status == "Unknown"),
        "{statuses:?}"
    );
}

#[test]
fn corpus_emission_round_trips_predicate_cases() {
    let program = elaborate(ARITHMETIC);
    let report = run_conformance(&program, &reference_only(3));
    let corpus = report.to_corpus(&program);
    let case_count: usize = report.predicates.iter().map(|p| p.cases.len()).sum();
    assert_eq!(corpus.cases.len(), case_count);
    for case in &corpus.cases {
        let expected = case.expected.clone().expect("expected true");
        assert_eq!(
            expected,
            vec![mncs_model::ExecutionValue::Boolean { value: true }]
        );
        assert_eq!(case.request.target.module, program.module);
    }
}
