# Standard-library bundle distribution (language side)

Stage F: the standard library lives in `mncs-stdlib`. This document
describes the bundle *mechanism* owned by the reference compiler; the
bundle *pin* (content) is owned by `mncs-stdlib`
(`dist/stdlib-bundle.json`, see `mncs-stdlib/docs/bundle.md`).

## Mechanism vs content

- **Format + resolver** (`crates/mncs-compiler/src/bundle.rs`): schema
  `mncs.stdlib-bundle/1`, per-module `sha256`, `bundle_identity` over
  sorted `(name, digest)` pairs, `MNCS_STDLIB_BUNDLE` resolution,
  fail-closed divergence (`MNE234`), in-process `pinned_bundle()`.
- **Canonical pin**: generated in `mncs-stdlib` via `./tools/regen.sh`
  (`mncs bundle generate` over `mncs-stdlib/library/`).
- **Vendored pin** (`stdlib-pin/stdlib-bundle.json`): lockfile copy so
  this repository builds and its in-process consumers resolve without
  a sibling checkout. Identity-checked at build and test time; update
  by copying the canonical pin (see `stdlib-pin/README.md`).

## Resolution

`MNCS_STDLIB_BUNDLE` names a bundle file for any resolving command.
The filesystem roots are `MNCS_LIBRARY_PATH` entries plus the
discovered stdlib root (`MNCS_STDLIB_ROOT`, else the `mncs-stdlib`
sibling of this checkout; set-but-empty disables the default for
hermetic invocations). Authority rules are unchanged:

- byte-identical content from pin and filesystem collapses to one
  authority — re-pinning a resolved tree is silent;
- divergent content for one identity fails closed as `MNE234`, naming
  both locators including `stdlib-bundle:{pin}:{module}`;
- only exact versioned names match the bundle (`mncs.std.chunk.v1`);
  unversioned requests keep flowing to the filesystem;
- an unreadable/unverifiable bundle fails loudly; unset means pure
  filesystem resolution.

Bundle envelopes carry a `Generated` origin naming pin and module so
import-failure chains identify the serving pin.

## Commands

```bash
# In mncs-stdlib: regenerate the canonical pin + manifest.
./tools/regen.sh
./tools/regen.sh --check

# Anywhere: verify a pin file.
mncs bundle verify <pin.json>

# Re-vendor here after the canonical pin changes.
cp ../mncs-stdlib/dist/stdlib-bundle.json stdlib-pin/stdlib-bundle.json
cargo test -p mncs-compiler --test stdlib_bundle
```

## In-process adoption

```rust
let bundle = mncs_compiler::bundle::pinned_bundle().expect("stdlib pin");
let front_end = compiler.front_end_with_resolver(envelope, &bundle.resolver());
```

## Compatibility

Bundle identities name content, never a path, timestamp, or branch.
Composition compatibility (which toolchain satisfies which stdlib) is
answered from `mncs-stdlib/stdlib-manifest.json`
(`requires_profile` vs supported profiles); the language capability
index (`docs/language-capabilities.json`) carries the projected
`stdlib` block. See `mncs-stdlib/docs/compatibility.md`.

## Non-goals

No registry, network fetch, constraint solving, or third-party
namespaces. The bundle distributes the first-party `mncs.*` tree as
content-addressed bytes.
