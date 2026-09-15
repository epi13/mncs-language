# Generated typed host bindings

`mncs abi` is the language-owned callable metadata source. The small
`scripts/generate_host_bindings.py` generator turns that metadata into
language-facing wrappers over the existing typed-call ABI:

```text
mncs abi <module.mncs> \
  → interface_identity + callable/composite metadata \
  → deterministic Python or Rust binding
```

The generated artifact records the host language, module identity, callable
interface identity, typed-call schema revision, generator version, and a
content identity over those inputs. Every call submits the expected interface
identity in-band. The runtime rejects a stale or legacy artifact before it
resolves arguments, so regeneration is required after a signature, record
field, enum variant, or return-type change.

The generator supports both one-record convenience calls and multi-argument
callables. Exact sequences and bounded views are represented by host
collections; byte sequences/views use `bytes` in Python and `Vec<u8>` in Rust,
while the runtime remains responsible for enforcing declared lengths and view
capacities. The binding therefore owns the transport spelling without
duplicating the language's semantic policy.

Python bindings use the trusted `mncs execute` entrypoint. Rust bindings use
`mncs-embed::Session` and decode generated nominal records and enums. Neither
binding owns semantic policy or family topology; they only provide typed host
ergonomics around language-owned metadata.
