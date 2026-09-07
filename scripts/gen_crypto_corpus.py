#!/usr/bin/env python3
"""Generate the bounded corpora for verify-only cryptography.

Emits two ExecutionCorpus JSON files (schema 0.1) matching mncs-model's
serde format:

- ``crypto-verify-corpus.json``: success cases. The digest and issuance
  vectors below come from the independent Python ``cryptography`` oracle
  (regenerate with the command in ``ORACLE``); the corpus pins the exact
  32 digest bytes and the boolean verdicts. A forged signature is
  ``false``, never a failure status.
- ``crypto-verify-reject-corpus.json``: malformed shapes. A 31-byte key
  and a 63-byte signature are ``invalid_request`` with pinned
  ``expected_status``; no effect is recorded and no value is produced.
  These cases fail the experiment run by design (refusals are
  observations, not passes).

Coverage is the views' runtime bytes: corpus arrays are exact-length,
never padded. Effect expectations pin the realized crypto observations
(kind/target/capability) with prohibition of anything unexpected.
Run from the repository root:

    python3 scripts/gen_crypto_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "examples.crypto_verify"

ORACLE = (
    "python3 -c \"from cryptography.hazmat.primitives.asymmetric.ed25519 "
    "import Ed25519PrivateKey; import hashlib; "
    "priv = Ed25519PrivateKey.from_private_bytes(bytes(range(32))); "
    "pub = priv.public_key().public_bytes_raw(); msg = b'atlas-issuance-07'; "
    "sig = priv.sign(msg); print(pub.hex()); print(sig.hex()); "
    "print(hashlib.sha256(msg).hexdigest())\""
)

MSG = bytes.fromhex("61746c61732d69737375616e63652d3037")
PUBKEY = bytes.fromhex(
    "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8"
)
SIG = bytes.fromhex(
    "f6639c7ad8727b70852a7f6f6c27cc8ee4a5b3071e6a57f127e290a1a86103f9"
    "be589ffcfccf932397a94ce535b02009ff4b8b0f8ba28d749f3d084274b80206"
)
DIGEST = bytes.fromhex(
    "9e664278d743d059d54a6ff854c47246e0c0ae4159629e2090350cd3fb485c29"
)
EMPTY_DIGEST = bytes.fromhex(
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
)
FORGED = SIG[:-1] + bytes([(SIG[-1] + 1) % 256])

assert len(MSG) == 17
assert len(PUBKEY) == 32
assert len(SIG) == 64
assert len(DIGEST) == 32


def view(raw):
    return {"sequence": {"values": [{"byte": {"value": b}} for b in raw]}}


def boolean(value):
    return {"boolean": {"value": value}}


def crypto_effect(kind, target):
    return {
        "expected_effects": [
            {"kind": kind, "target": target, "capability": "verifier"}
        ],
        "prohibit_unexpected_effects": True,
    }


def no_effects():
    return {
        "expected_effects": [],
        "prohibit_unexpected_effects": True,
    }


def case(case_id, function, arguments, expected, effects, **extra):
    body = {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": list(arguments),
            "step_budget": 8192,
        },
    }
    if expected is not None:
        body["expected"] = [expected]
    body.update(effects)
    body.update(extra)
    return body


success = [
    case(
        "digest-known",
        "digest_message",
        [view(MSG)],
        view(DIGEST),
        crypto_effect("sha256_digest", "sha256_digest"),
    ),
    case(
        "digest-empty",
        "digest_message",
        [view(b"")],
        view(EMPTY_DIGEST),
        crypto_effect("sha256_digest", "sha256_digest"),
    ),
    case(
        "issuance-ok",
        "check_issuance",
        [view(PUBKEY), view(MSG), view(SIG)],
        boolean(True),
        crypto_effect("ed25519_verify", "ed25519_verify"),
    ),
    case(
        "issuance-forged",
        "check_issuance",
        [view(PUBKEY), view(MSG), view(FORGED)],
        boolean(False),
        crypto_effect("ed25519_verify", "ed25519_verify"),
    ),
]

reject = [
    case(
        "short-key",
        "check_issuance",
        [view(PUBKEY[:31]), view(MSG), view(SIG)],
        None,
        no_effects(),
        expected_status="invalid_request",
    ),
    case(
        "short-signature",
        "check_issuance",
        [view(PUBKEY), view(MSG), view(SIG[:63])],
        None,
        no_effects(),
        expected_status="invalid_request",
    ),
]

with open(os.path.join(OUT, "crypto-verify-corpus.json"), "w") as handle:
    json.dump(
        {"schema_version": "0.1", "name": "crypto-verify", "cases": success},
        handle,
        indent=1,
    )
    handle.write("\n")

with open(os.path.join(OUT, "crypto-verify-reject-corpus.json"), "w") as handle:
    json.dump(
        {
            "schema_version": "0.1",
            "name": "crypto-verify-reject",
            "cases": reject,
        },
        handle,
        indent=1,
    )
    handle.write("\n")

print(f"wrote {len(success)} success and {len(reject)} reject cases")
