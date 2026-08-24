# Source Profile 0.6: Module Imports (Experimental)

Profile 0.6 includes all Profile 0.5 programs unchanged and adds module import
declarations, an experimental step toward RFC 0014's evidence-bearing binding
model.

## Syntax

```mncs
mncs 0.6;

module app.study;

use lib.evidence;

fn decide(first: Verdict) -> (result: Verdict) {
    return combine(first, first);
}
```

`use <dotted.name>;` declarations appear after the module declaration and
before every other declaration. In profiles below 0.6 the `use` keyword is an
ordinary identifier; a use-shaped line produces diagnostic `MNP136`, not
silence.

## Semantics

- **Elaboration-time linking.** Each imported module is elaborated
  independently against the same resolver; its exported declarations bind into
  the importing module's namespace. There is one semantic program after
  elaboration, so HIR, SSA, obligations, backends, and execution are unchanged
  by linking.
- **Names identify, semantics bind.** The imported name is a discovery route.
  Compatibility is established only by successful elaboration of the resolved
  module (RFC 0014, principle 1).
- **One namespace after binding.** Imported functions, finite types, and
  record types are referenced by their bare names. Collisions fail closed:
  - duplicate `use` of one module: `MNE170`;
  - import cycles, including self-import: `MNE171`;
  - unresolvable or unparsable dependency: `MNE173`, `MNE172`;
  - finite-type name collision: `MNE174`;
  - record-type name collision: `MNE175`;
  - function name collision: `MNE176`.
  A diamond re-export of the *same* identity binds once and is accepted.
- **Identity provenance.** Declarations keep identities anchored to their
  declaring module: functions carry `home_module`, types keep their declaring
  namespace in their identities. A linked declaration therefore has one
  stable identity in every importing program. `Program.dependencies` records
  the direct import closure as module identities, sorted.
- **Authority closure crosses the boundary.** A caller must re-declare every
  capability and matching effect pair of any callee, local or imported;
  undeclared authority across a module boundary fails with `MNE134`. Role
  boundaries survive composition (RFC 0014, principle 2).
- **Acyclicity.** Calls remain acyclic; imported callees are leaves already
  validated inside their own modules.

## Resolution contract

The compiler core consumes imports through the `ModuleResolver` trait; it
never touches the filesystem itself. Hosts choose their own layout rules:

- the research CLI resolves names relative to the importing file's directory,
  trying `<full.dotted.path>.mncs` then `<tail>.mncs`;
- language-service hosts may resolve against resident workspace documents.

A resolution miss is always a diagnostic (`MNE173`) in the importing module.

## Non-goals for this profile

No qualified call syntax, no selective/re-named imports, no interface
signatures or requirement/provider binding (RFC 0014 components), no version
selection. These remain future work; nothing here forecloses them.
