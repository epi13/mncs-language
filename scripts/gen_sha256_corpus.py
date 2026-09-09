#!/usr/bin/env python3
"""Generate the SHA-256 corpus for mncs.std.sha256.v1.

Round constants K are derived from first principles (first 32 bits of
the fractional parts of the cube roots of the first 64 primes, computed
in 50-digit decimal) and cross-checked against four known prefix
constants, so the corpus does not merely parrot an implementation. The
expected digests are NIST/SHA-256 oracles from hashlib:

- "abc" (single block through init/update/finalize);
- the 156-byte examples/fs-fixture/sub/b.txt in a 64+64+28 chunking;
- the same bytes in a 30+63+63 chunking (chunking independence: the
  same expected digest under a different split).

Run from the repository root:

    python3 scripts/gen_sha256_corpus.py
"""

import hashlib
import json
import os
from decimal import Decimal, getcontext

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.sha256.v1"


def first_primes(n):
    primes = []
    candidate = 2
    while len(primes) < n:
        if all(candidate % p for p in primes):
            primes.append(candidate)
        candidate += 1
    return primes


def round_constants():
    getcontext().prec = 50
    consts = []
    for prime in first_primes(64):
        root = Decimal(prime).ln() / Decimal(3)
        root = root.exp()
        frac = root - int(root)
        consts.append(int(frac * (1 << 32)))
    return consts


K = round_constants()
assert K[0] == 0x428A2F98, hex(K[0])
assert K[1] == 0x71374491, hex(K[1])
assert K[2] == 0xB5C0FBCF, hex(K[2])
assert K[63] == 0xC67178F2, hex(K[63])


def u32(value):
    return {"integer": {"value": value, "type": {"bits": 32, "signed": False}}}


def u64(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def byte_seq(data: bytes):
    return {"sequence": {"values": [{"byte": {"value": b}} for b in data]}}


def digest_bytes(data: bytes):
    return {"sequence": {"values": [{"byte": {"value": b}} for b in hashlib.sha256(data).digest()]}}


def k_arg():
    return {"sequence": {"values": [u32(v) for v in K]}}


def case(cid, function, arguments, expected):
    return {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": arguments,
            "step_budget": 1000000,
        },
        "expected": [expected],
    }


def split(data: bytes, sizes):
    chunks = []
    offset = 0
    for size in sizes:
        chunks.append(data[offset:offset + size])
        offset += size
    assert offset == len(data)
    return chunks


def main():
    blob = open(os.path.join(ROOT, "examples", "fs-fixture", "sub", "b.txt"), "rb").read()
    assert len(blob) == 156, len(blob)

    def blocks_call(cid, sizes):
        chunks = split(blob, sizes)
        args = []
        for piece in chunks:
            args.extend([byte_seq(piece), u64(len(piece))])
        return case(cid, "candidate_sha256_blocks", args + [k_arg()],
                    digest_bytes(blob))

    cases = [
        case("sha256-abc", "candidate_sha256_abc",
             [byte_seq(b"abc"), u64(3), k_arg()],
             digest_bytes(b"abc")),
        blocks_call("sha256-blocks-64-64-28", [64, 64, 28]),
        blocks_call("sha256-blocks-30-63-63", [30, 63, 63]),
    ]
    path = os.path.join(OUT, "sha256-corpus.json")
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(
            {"schema_version": "0.1", "name": "sha256", "cases": cases},
            handle,
            indent=1,
        )
        handle.write("\n")
    print(f"wrote {path} ({len(cases)} cases)")
    print("abc   :", hashlib.sha256(b"abc").hexdigest())
    print("b.txt :", hashlib.sha256(blob).hexdigest())


if __name__ == "__main__":
    main()
