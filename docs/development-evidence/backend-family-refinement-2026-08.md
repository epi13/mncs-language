# Bounded observe–compare–withhold experiment — 2026-08

Forge/search may propose backend realizations. It does not decide MNCS semantics.

## Observe

CRE-1/2/3 already exist as frozen source+corpus identities. The language pressure was
backend plurality: WASM must not remain the only executable shape.

## Localize

The selected-SSA envelope (checked/wrapping integers, finite values, calls, Profile 0.4
bounded iteration) is the backend boundary. Missing execution is not a semantic rewrite.

## Propose

Candidates:

- `mncs-llvm-ir` + external clang
- `mncs-c11` + external clang/gcc
- `mncs-cranelift` host JIT
- retain `mncs-portable-wasm-mvp` and `mncs-research-bytecode` as controls

## Compile / execute / verify

| Candidate | CRE-1 | CRE-2 | CRE-3 | Decision |
| --- | --- | --- | --- | --- |
| WASM | PASS | PASS | PASS | retain as portable research realization |
| bytecode | PASS | PASS | PASS | retain as second shape / control |
| LLVM IR | PASS | PASS | PASS | accept as experimental external realization |
| C11 | PASS | PASS | PASS | accept as experimental portability realization |
| Cranelift CRE JIT | FAIL / mapping error | FAIL | ERROR (now panic-fixed, execution still untrusted) | **reject for CRE execution promotion** |

Rejection of Cranelift CRE execution is a successful experiment: a candidate that cannot
discharge execution obligations is not promoted, even though lowering exists.

## Promotion

No candidate was promoted to MNCS semantics or to a privileged backend. Cross-backend
PASS on CRE-1/2/3 for WASM/bytecode/LLVM/C11 is bounded observational agreement.
`UNKNOWN` exact instruction cost is retained.

Forge did not gain semantic authority.

This earlier observe/compare/withhold record is superseded for the Roadmap 0.5 completion gate by
the sealed `BoundedRefinementCycle` experiment in `roadmap-0.5-2026-08.md`. The earlier Cranelift
rejection remains useful host-scoped negative evidence.
