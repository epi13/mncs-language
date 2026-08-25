//! Debug utility: lower a .mncs source through a text backend and write the
//! module to stdout (`--backend c11|llvm`).

use mncs_codegen::BackendAdapter;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: dump_text <file.mncs> [c11|llvm]")?;
    let backend = args.next().unwrap_or_else(|| "llvm".to_owned());
    let src = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let envelope =
        mncs_syntax::SourceEnvelope::inline(mncs_syntax::SourceArtifactKind::Program, "probe", src);
    let front = mncs_compiler::ReferenceCompiler::default().front_end(envelope);
    let program = front.program.ok_or("elaboration failed")?;
    let ssa = program.lower_to_ssa().map_err(|e| e.to_string())?;
    let selected = mncs_codegen::selected_ssa_ref(&ssa);
    let result = match backend.as_str() {
        "c11" => {
            let plan = mncs_codegen::C11Adapter.plan(selected.clone());
            mncs_codegen::C11Adapter.lower(&program, &ssa, selected, &plan)
        }
        _ => {
            let plan = mncs_codegen::LlvmAdapter.plan(selected.clone());
            mncs_codegen::LlvmAdapter.lower(&program, &ssa, selected, &plan)
        }
    };
    match result.artifact {
        Some(artifact) => {
            print!(
                "{}",
                String::from_utf8_lossy(&artifact.bytes().map_err(|e| e)?)
            );
            Ok(())
        }
        None => {
            for diagnostic in result.diagnostics {
                eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            }
            Err("lowering refused".to_owned())
        }
    }
}
