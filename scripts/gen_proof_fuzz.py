#!/usr/bin/env python3
"""Generate the adversarial/differential fuzz corpus for the RFC 0007 kernel.

Cases carry no `expected` values: the differential suite executes them on
every backend and compares the returned verdict codes against the
independent Rust reference checker. Any disagreement is a checker bug;
any non-`returned` status is a kernel robustness bug (the kernel must be
total and fail closed, never trap).

Generation is seeded and deterministic so failures reproduce exactly.
"""

import json
import random
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from gen_proof_corpus import byte, cell, pad, TAGS

CASE_COUNT = 120


def fuzz_cell(rng, count):
    tag = rng.choice(TAGS)
    # Bias arguments toward in-range indices with frequent edge violations:
    # negatives, self references, forward references, and wild values.
    def arg():
        roll = rng.random()
        if roll < 0.55:
            return rng.randrange(0, count)
        if roll < 0.65:
            return count
        if roll < 0.72:
            return count + rng.randrange(1, 4)
        if roll < 0.80:
            return -rng.randrange(1, 4)
        if roll < 0.90:
            return rng.randrange(0, 5)
        return rng.randrange(-2, 20)

    if tag == "Universe":
        return cell(tag, rng.choice([0, 1, 2, 3, 3, 4, -1, 9]))
    return cell(tag, arg(), arg(), arg(), arg())


def fuzz_case(rng, number):
    count = rng.choice([0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16])
    cells = [fuzz_cell(rng, max(count, 1)) for _ in range(16)]
    # Occasionally plant a well-formed proof so PASS/UNKNOWN paths trigger.
    # A plant owns the whole active prefix: trailing garbage inside the
    # prefix would (correctly) fail closed, so plants set an exact count.
    roll = rng.random()
    if roll < 0.15:
        cells[0] = cell("Nat")
        cells[1] = cell("Zero")
        cells[2] = cell("Eq", 0, 1, 1)
        cells[3] = cell("Refl", 1, 2)
        count, proof, prop = 4, 3, 2
    elif roll < 0.25:
        cells[0] = cell("Universe", 0)
        cells[1] = cell("Nat")
        cells[2] = cell("Pi", 1, 1)
        cells[3] = cell("Var", 2)
        cells[4] = cell("Eq", 1, 3, 3)
        cells[5] = cell("Pi", 4, 4)
        cells[6] = cell("Var", 5)
        cells[7] = cell("Lambda", 4, 6, 5)
        count, proof, prop = 8, 7, 5
    else:
        proof = rng.choice([0, max(count - 1, 0), count, count + 1, rng.randrange(0, 17)])
        prop = rng.choice([0, max(count - 1, 0), count, rng.randrange(0, 17)])
    return {
        "id": f"fuzz-{number:03d}",
        "request": {
            "schema_version": "0.1",
            "target": {
                "module": "mncs.core.proof_check.v1",
                "function": "check_proof_code",
            },
            "arguments": [
                {"sequence": {"values": cells}},
                byte(count),
                byte(proof),
                byte(prop),
            ],
            "step_budget": 65536,
        },
        "expected_status": "returned",
    }


def main():
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 20260905
    rng = random.Random(seed)
    cases = [fuzz_case(rng, number) for number in range(CASE_COUNT)]
    corpus = {
        "schema_version": "0.1",
        "name": "proof-kernel-fuzz",
        "seed": seed,
        "cases": cases,
    }
    path = sys.argv[1] if len(sys.argv) > 1 else "-"
    text = json.dumps(corpus, indent=1) + "\n"
    if path == "-":
        sys.stdout.write(text)
    else:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        print(f"wrote {len(cases)} fuzz cases (seed {seed}) to {path}")


if __name__ == "__main__":
    main()
