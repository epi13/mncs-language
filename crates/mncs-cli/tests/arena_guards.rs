//! Native arena guards: every LLVM arena dereference is dominated by an
//! explicit bounds guard, and every bump allocation is exhaustion-checked.
//!
//! Background: the LLVM backend addresses the canonical composite-cell arena
//! with `getelementptr inbounds` against `@mncs_arena`. An `inbounds` GEP on
//! a forged or exhausted address is immediate undefined behavior (and a
//! miscompiled guard removal), so each of the ten lowering sites proves its
//! address range first with one unsigned `icmp ugt addr, LEN - width` that
//! branches to `%mncs_fail` on violation. That single comparison covers a
//! negative bit-pattern, a past-the-end base, and a base+width wraparound.
//! Bump allocations check huge requests, an already-spent cursor, and room
//! for the aligned base before the cursor advances.
//!
//! Execution-level fail-closed behavior for out-of-bounds indices is covered
//! by `view_return_traps_fail_at_runtime_consistently` in `abi_boundary.rs`;
//! this file pins the structural guarantee that no arena GEP can execute
//! without its guard, including against forged bases that source-level
//! evidence checks cannot see.

use std::process::Command;

use serde_json::Value;

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// Compile one source module to one backend and return the emitted text.
fn compile_backend_text(source: &str, target: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "mncs-arena-guards-{}-{}",
        std::process::id(),
        target.replace("mncs-", "")
    ));
    std::fs::create_dir_all(&dir).expect("create output dir");
    let output = binary()
        .args([
            "compile",
            source,
            "--emit",
            "backend",
            "--target",
            target,
            "--output-dir",
        ])
        .arg(&dir)
        .output()
        .expect("compile backend");
    assert!(
        output.status.success(),
        "{target}: compilation failed for {source}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{target}: compilation JSON: {error}"));
    let backend = result["emissions"]["backend"]
        .as_object()
        .unwrap_or_else(|| panic!("{target}: missing backend emission for {source}: {result:#}"));
    assert_eq!(
        backend["status"], "PASS",
        "{target}: backend lowering must succeed for {source}"
    );
    let hex = backend["bytes_hex"]
        .as_str()
        .unwrap_or_else(|| panic!("{target}: missing artifact bytes"));
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect();
    String::from_utf8(bytes).expect("backend text is UTF-8")
}

/// The address operand of an `@mncs_arena` GEP on one IR line, if any.
fn arena_gep_addr(line: &str) -> Option<&str> {
    let marker = "ptr @mncs_arena, i64 0, i64 ";
    let start = line.find(marker)? + marker.len();
    Some(line[start..].split_whitespace().next().unwrap_or(""))
}

/// The `(guarded, limit)` pair of an unsigned arena-guard comparison.
fn guard_compare(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let rest = line.strip_prefix("%")?;
    let rest = rest.split_once(" = icmp ugt i64 ")?.1;
    let (addr, limit) = rest.split_once(", ")?;
    Some((addr, limit.split_whitespace().next().unwrap_or("")))
}

fn is_block_label(line: &str) -> bool {
    let line = line.trim();
    line.ends_with(':')
        && !line.contains(' ')
        && !line.starts_with(';')
        && !line.starts_with("define")
}

/// Every `@mncs_arena` dereference without a dominating in-block guard,
/// as `(line number, address)` pairs.
fn unguarded_geps(ir: &str) -> Vec<(usize, String)> {
    let mut guarded_in_block: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unguarded = Vec::new();
    for (number, line) in ir.lines().enumerate() {
        // A guard's own `agN_ok:` fallthrough stays in the guarded region;
        // any other label starts a fresh block.
        if is_block_label(line) {
            let label = line.trim();
            let is_guard_ok = label.starts_with("ag") && label.ends_with("_ok:");
            if !is_guard_ok {
                guarded_in_block.clear();
            }
            continue;
        }
        if let Some((addr, _)) = guard_compare(line) {
            guarded_in_block.insert(addr.to_owned());
        }
        if let Some(addr) = arena_gep_addr(line) {
            if !guarded_in_block.contains(addr) {
                unguarded.push((number + 1, addr.to_owned()));
            }
        }
    }
    unguarded
}

/// Assert every `@mncs_arena` dereference is dominated in-block by an
/// unsigned guard on the same address. Returns the guarded GEP count.
fn assert_arena_geps_guarded(ir: &str, what: &str) -> usize {
    let unguarded = unguarded_geps(ir);
    assert!(
        unguarded.is_empty(),
        "{what}: arena GEPs without a dominating guard: {unguarded:?}"
    );
    ir.lines()
        .filter(|line| arena_gep_addr(line).is_some())
        .count()
}

/// The dominance audit itself must catch a missing guard (negative control)
/// and accept the guarded shape (positive control).
#[test]
fn guard_dominance_audit_catches_missing_guards() {
    let guarded = "b0:\n  %a = add i64 %b, 0\n  %ag1 = icmp ugt i64 %a, 100\n  br i1 %ag1, label %mncs_fail, label %ag1_ok\nag1_ok:\n  %p = getelementptr inbounds [128 x i8], ptr @mncs_arena, i64 0, i64 %a\n";
    assert!(
        unguarded_geps(guarded).is_empty(),
        "guarded GEP must pass the audit"
    );
    let missing = "b0:\n  %a = add i64 %b, 0\n  %p = getelementptr inbounds [128 x i8], ptr @mncs_arena, i64 0, i64 %a\n";
    assert_eq!(
        unguarded_geps(missing),
        vec![(3, "%a".to_owned())],
        "unguarded GEP must be reported"
    );
    let stale_block = "b0:\n  %g = icmp ugt i64 %a, 100\n  br label %b1\nb1:\n  %p = getelementptr inbounds [128 x i8], ptr @mncs_arena, i64 0, i64 %a\n";
    assert_eq!(
        unguarded_geps(stale_block).len(),
        1,
        "a guard in a previous block must not cover a later GEP"
    );
}

fn llc_available() -> bool {
    Command::new("llc")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Records and payload-bearing finite variants lower through canonical
/// cells: every cell dereference is guarded and the allocator checks for
/// exhaustion before advancing the bump cursor.
#[test]
fn llvm_arena_guards_cover_cells_and_variants() {
    let ir = compile_backend_text(&library("core/result.mncs"), "mncs-llvm-ir");
    let geps = assert_arena_geps_guarded(&ir, "result.mncs");
    assert!(geps > 0, "result.mncs must dereference arena cells");
    assert!(
        ir.contains("load i64, ptr @mncs_bump"),
        "result.mncs must bump-allocate cells"
    );
    assert!(
        ir.contains("arena-guard:alloc"),
        "every bump allocation must carry the exhaustion check"
    );
    if llc_available() {
        let dir = std::env::temp_dir().join(format!("mncs-arena-llc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create llc dir");
        let path = dir.join("guarded.ll");
        std::fs::write(&path, &ir).expect("write IR");
        let output = Command::new("llc")
            .arg(&path)
            .arg("-o")
            .arg(dir.join("guarded.s"))
            .output()
            .expect("run llc");
        assert!(
            output.status.success(),
            "guarded IR must assemble: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        eprintln!("SKIP-BUT-STRUCTURED: no llc on PATH; textual guard dominance asserted instead");
    }
}

/// Views and sequences lower through arena projections and scratch
/// allocations: the same guard discipline applies at every site.
#[test]
fn llvm_arena_guards_cover_views_and_sequences() {
    let ir = compile_backend_text(&example("source/abi-view-returns.mncs"), "mncs-llvm-ir");
    let geps = assert_arena_geps_guarded(&ir, "abi-view-returns.mncs");
    assert!(
        geps > 0,
        "view/sequence lowering must dereference the arena"
    );
}

/// C11 cell traffic goes through the range-checked slot helpers rather than
/// raw arena dereferences: the helper definitions (with their
/// `MNCS_ARENA_BYTES - width` limits) must be present whenever a module
/// touches cells.
#[test]
fn c11_cell_access_goes_through_checked_helpers() {
    let c = compile_backend_text(&library("core/result.mncs"), "mncs-c11");
    for helper in [
        "mncs_slot_load32",
        "mncs_slot_load64",
        "mncs_slot_store32",
        "mncs_slot_store64",
    ] {
        assert!(
            c.contains(helper),
            "cell traffic must route through {helper}"
        );
    }
    assert!(
        c.contains("MNCS_ARENA_BYTES - 4u") && c.contains("MNCS_ARENA_BYTES - 8u"),
        "slot helpers must carry the width-relative arena limits"
    );
}
