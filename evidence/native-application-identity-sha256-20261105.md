# Native application identity SHA-256

## Responsibility moved

`mncs run-app` now computes the SHA-256 digest placed in
`ApplicationContext.application_identity` by calling
`mncs.std.sha256.v1::digest_standard` through the admitted MNCS EmbedSession.
The host still bounds and transports the identity bytes and converts the
returned byte sequence to the existing context ABI. It no longer computes the
digest. The direct `sha2` runtime dependency was removed from `mncs-cli`; the
test oracle and build metadata retain their independent uses of `sha2`.

The existing application descriptor contract bounds identity strings to
1–1024 UTF-8 bytes. `digest_standard` handles that full bound, empty input,
SHA-256 padding boundaries, and multiple blocks. Its runtime-length block view
avoids executing unused blocks. The hasher uses the shared bounded native
artifact cache under identity `mncs-application-context-sha256-v1`, so repeated
application runs reuse both the app and digest artifacts.

## Verification

- `sha256_bounded_digest_matches_oracle_on_executable_backends`: passed on
  Research Bytecode, Portable WASM MVP, C11, LLVM IR, and Cranelift. Eleven
  inputs (lengths 0, 1, 55, 56, 63, 64, 65, 127, 128, 129, and 1024) matched
  Rust `sha2` expectations on every backend. Maximum reported Research
  Bytecode work was 346,017 steps, below the production budget of 500,000.
  Backend step counters have backend-specific meanings and are not directly
  comparable.
- `run_app_supplies_native_sha256_identity_at_the_application_boundary`: passed.
  The real `run-app` consumer returned the independently computed digest for a
  1024-byte identity on two runs; the second run reported cache hits for both
  native artifacts.
- `scripts/test_generate_language_capabilities.py`: passed after regenerating
  the capability index (`sha256:0f8711fb39044765f4a4520c75ed1447b3beb7cf725c293f6961ccfa308dd142`).
- `cargo fmt --all --check` and `git diff --check`: passed.
- The Environment-declared full `cargo test --package mncs-cli` capability did
  not finish. Session `ses_3f79ff1bc6018808`, invocation event 46, records
  status `timeout` after the capability's 600-second limit (600,648 ms held).
  The invocation produced no result artifacts, so this run is incomplete and
  is not counted as a pass or a test failure.

The focused executable-backend matrix took 197.51 seconds; the production
consumer test took about 15 seconds. The Environment-created linked-worktree
Cargo target was removed after its timed-out process ended.
