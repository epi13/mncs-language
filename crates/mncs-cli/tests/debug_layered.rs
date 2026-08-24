use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn debug_layered_loop() {
    let source = std::fs::read_to_string(
        "/home/epi13/Documents/Projects/RAVEL/mncs/workspace/ravel_loop.mncs",
    )
    .unwrap();
    let envelope = mncs_syntax::SourceEnvelope::inline(
        mncs_syntax::SourceArtifactKind::Program,
        "debug.loop",
        source,
    );
    let front_end = mncs_compiler::ReferenceCompiler::default().front_end(envelope);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.unwrap();
    let ssa = program.lower_to_ssa().unwrap();
    let corpus: mncs_model::ExecutionCorpus = serde_json::from_slice(
        &std::fs::read("/home/epi13/Documents/Projects/RAVEL/mncs/corpus/ravel-loop-corpus.json")
            .unwrap(),
    )
    .unwrap();

    println!(
        "finite_types={:?}",
        program
            .finite_types
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>()
    );
    for rt in &program.record_types {
        println!(
            "record {} fields={:?} identity={}",
            rt.name,
            rt.fields
                .iter()
                .map(|f| (f.name.clone(), f.field_type.clone()))
                .collect::<Vec<_>>(),
            rt.identity
        );
    }
    for case_ in &corpus.cases {
        if !case_.id.starts_with("evaluate") {
            continue;
        }
        let body = mncs_model::execute_with_policy(&program, &case_.request);
        let ssa_result = mncs_model::execute_ssa_module(&program, &ssa, &case_.request);
        println!(
            "{}\n  body={:?} reason={:?}\n  ssa={:?}",
            case_.id,
            body.status,
            body.failure.as_ref().map(|f| f.reason.clone()),
            ssa_result.status,
        );
        let _ = Value::Null;
    }
}
