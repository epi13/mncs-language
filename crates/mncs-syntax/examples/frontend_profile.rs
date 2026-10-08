use mncs_syntax::{lex, parse, SourceArtifactKind, SourceEnvelope, SourceOrigin, SourceOriginKind};
use std::{env, fs, path::PathBuf, time::Instant};

fn main() {
    let mut args = env::args_os().skip(1);
    let mode = args
        .next()
        .and_then(|value| value.into_string().ok())
        .expect("usage: frontend-profile <lex|parse> <source-path>");
    let path = PathBuf::from(
        args.next()
            .expect("usage: frontend-profile <lex|parse> <source-path>"),
    );
    assert!(
        args.next().is_none(),
        "usage: frontend-profile <lex|parse> <source-path>"
    );
    assert!(
        matches!(mode.as_str(), "lex" | "parse"),
        "mode must be lex or parse"
    );

    let read_started = Instant::now();
    let text = fs::read_to_string(&path).expect("read UTF-8 source");
    let read_us = read_started.elapsed().as_micros();
    let bytes = text.len();
    let envelope_started = Instant::now();
    let envelope = SourceEnvelope::new(
        SourceArtifactKind::Program,
        path.display().to_string(),
        SourceOrigin {
            kind: SourceOriginKind::Path,
            locator: Some(path.display().to_string()),
        },
        text,
    );
    let envelope_us = envelope_started.elapsed().as_micros();

    let phase_started = Instant::now();
    let result = match mode.as_str() {
        "lex" => {
            let output = lex(&envelope);
            serde_json::json!({
                "token_count": output.tokens.len(),
                "diagnostic_count": output.diagnostics.len(),
                "source_identity": output.source_identity,
            })
        }
        "parse" => {
            let output = parse(&envelope);
            serde_json::json!({
                "ast_present": output.ast.is_some(),
                "diagnostic_count": output.diagnostics.len(),
                "source_identity": output.cst.source_identity,
            })
        }
        _ => unreachable!(),
    };
    let phase_us = phase_started.elapsed().as_micros();
    println!(
        "{}",
        serde_json::json!({
            "schema": "mncs.syntax-frontend-profile/1",
            "mode": mode,
            "path": path,
            "source_bytes": bytes,
            "read_us": read_us,
            "envelope_seal_us": envelope_us,
            "source_identity": envelope.identity,
            "phase_us": phase_us,
            "result": result,
        })
    );
}
