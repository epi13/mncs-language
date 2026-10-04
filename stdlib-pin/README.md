# Vendored stdlib pin (lockfile)

`stdlib-bundle.json` is a **pinned dependency**, not a canonical copy.
The canonical pin lives in the `mncs-stdlib` repository
(`dist/stdlib-bundle.json`); this file vendors it so the reference
compiler builds and its in-process consumers (`pinned_bundle()`) work
without a sibling checkout.

Identity-checked, never trusted:

- the build embeds these exact bytes (`mncs-compiler/src/bundle.rs`);
- `mncs-compiler/tests/stdlib_bundle.rs` verifies the pin is
  self-consistent and — when an `mncs-stdlib` checkout is available —
  byte-identical to the canonical pin;
- any divergence fails closed.

## Update procedure

```bash
cp ../mncs-stdlib/dist/stdlib-bundle.json stdlib-pin/stdlib-bundle.json
cargo test -p mncs-compiler --test stdlib_bundle
```

Commit the result. The pin names content (`mncs:stdlib-bundle:{hex}`),
so review the identity change, not the bytes.
