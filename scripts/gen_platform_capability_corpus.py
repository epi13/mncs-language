#!/usr/bin/env python3
"""Generate the bounded corpus for mncs.std.platform.v1 leaf decisions.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Every arm of the leaf predicates (os/arch/libc/cuda/resources/version/
flag) is covered, including the campaign's pressure points: unknown
observations satisfying nothing, CUDA compute floors, RISC-V emulation
paths, and musl/GNU distinction.
Run from the repository root:

    python3 scripts/gen_platform_capability_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.platform.v1"


def finite(type_name, variant_name, discriminant):
    return {
        "finite": {
            "type_identity": f"mncs:0.2:finite-type:{MODULE}::{type_name}",
            "variant_identity": f"mncs:0.2:finite-variant:{MODULE}::{type_name}::{variant_name}",
            "discriminant": discriminant,
        }
    }


def integer(value, bits=32, signed=True):
    return {"integer": {"value": value, "type": {"bits": bits, "signed": signed}}}


def boolean(value):
    return {"boolean": {"value": value}}


def case(case_id, function, arguments, expected, step_budget=1024):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": list(arguments),
            "step_budget": step_budget,
        },
        "expected": [expected],
    }


REQ_OS = {"Any": 0, "Linux": 1, "Windows": 2, "MacOs": 3}
OS = {"Linux": 0, "Windows": 1, "MacOs": 2, "Unknown": 3}
REQ_ARCH = {"Any": 0, "X86_64": 1, "Aarch64": 2, "Riscv64": 3, "X86": 4, "Arm32": 5}
ARCH = {"X86_64": 0, "Aarch64": 1, "Riscv64": 2, "X86": 3, "Arm32": 4, "Unknown": 5}
REQ_LIBC = {"Any": 0, "Gnu": 1, "Musl": 2, "WasiC": 3, "WinCrt": 4}
LIBC = {"Gnu": 0, "Musl": 1, "WasiC": 2, "WinCrt": 3, "Unknown": 4}
ACCEL = {"None": 0, "Cuda": 1, "Rocm": 2, "Metal": 3, "Unknown": 4}
EXEC = {"Native": 0, "Emulated": 1}
ENVELOPE = {"Any": 0, "Exact": 1, "AtLeast": 2, "CompatibleMinor": 3}


def main():
    cases = []
    # OS: concrete match, concrete mismatch, unknown satisfies nothing,
    # Any satisfies everything including unknown observations.
    for req, got, expected in [
        ("Linux", "Linux", True),
        ("Linux", "Windows", False),
        ("Windows", "Windows", True),
        ("Windows", "Linux", False),
        ("MacOs", "MacOs", True),
        ("Linux", "Unknown", False),
        ("Windows", "Unknown", False),
        ("Any", "Linux", True),
        ("Any", "Unknown", True),
    ]:
        cases.append(
            case(
                f"os-{req.lower()}-{got.lower()}",
                "candidate_os",
                [finite("ReqOs", req, REQ_OS[req]), finite("Os", got, OS[got])],
                boolean(expected),
            )
        )
    # Arch: native match, mismatch, emulation path (allowed + offered +
    # emulated execution), and every refusal shape around it.
    for cid, req, got, allow, emulates, execmode, expected in [
        ("x86-native", "X86_64", "X86_64", False, False, "Native", True),
        ("x86-mismatch", "X86_64", "Aarch64", False, False, "Native", False),
        ("arm-native", "Aarch64", "Aarch64", False, False, "Native", True),
        ("riscv-native", "Riscv64", "Riscv64", False, False, "Native", True),
        ("riscv-emulated", "Riscv64", "X86_64", True, True, "Emulated", True),
        ("riscv-emul-forbidden", "Riscv64", "X86_64", False, True, "Emulated", False),
        ("riscv-emul-unoffered", "Riscv64", "X86_64", True, False, "Emulated", False),
        ("riscv-emul-native-exec", "Riscv64", "X86_64", True, True, "Native", False),
        ("native-emul-without-allow", "X86_64", "X86_64", False, False, "Emulated", False),
        ("native-emul-with-allow", "X86_64", "X86_64", True, False, "Emulated", True),
        ("x86-unknown", "X86_64", "Unknown", False, False, "Native", False),
        ("any-unknown", "Any", "Unknown", False, False, "Native", True),
    ]:
        cases.append(
            case(
                f"arch-{cid}",
                "candidate_arch",
                [
                    finite("ReqArch", req, REQ_ARCH[req]),
                    finite("Arch", got, ARCH[got]),
                    boolean(allow),
                    boolean(emulates),
                    finite("ExecMode", execmode, EXEC[execmode]),
                ],
                boolean(expected),
            )
        )
    # Libc: musl/GNU distinction is load-bearing for Alpine pressure.
    for req, got, expected in [
        ("Gnu", "Gnu", True),
        ("Gnu", "Musl", False),
        ("Musl", "Musl", True),
        ("Musl", "Gnu", False),
        ("Gnu", "Unknown", False),
        ("Any", "Musl", True),
        ("Any", "Unknown", True),
    ]:
        cases.append(
            case(
                f"libc-{req.lower()}-{got.lower()}",
                "candidate_libc",
                [finite("ReqLibc", req, REQ_LIBC[req]), finite("Libc", got, LIBC[got])],
                boolean(expected),
            )
        )
    # CUDA: no requirement always holds; otherwise accelerator must be
    # CUDA at or above the compute floor.
    for cid, require, rmaj, rmin, got, gmaj, gmin, expected in [
        ("unrequired", False, 6, 1, "Unknown", 0, 0, True),
        ("floor-met", True, 6, 1, "Cuda", 6, 1, True),
        ("floor-exceeded", True, 6, 1, "Cuda", 8, 0, True),
        ("floor-missed", True, 7, 0, "Cuda", 6, 1, False),
        ("absent", True, 6, 1, "None", 0, 0, False),
        ("unknown", True, 6, 1, "Unknown", 0, 0, False),
        ("wrong-accel", True, 6, 1, "Rocm", 0, 0, False),
    ]:
        cases.append(
            case(
                f"cuda-{cid}",
                "candidate_cuda",
                [
                    boolean(require),
                    integer(rmaj),
                    integer(rmin),
                    finite("Accel", got, ACCEL[got]),
                    integer(gmaj),
                    integer(gmin),
                ],
                boolean(expected),
            )
        )
    # Resources: floors compare, never match as strings.
    for cid, mem, cpu, gmem, gcpu, expected in [
        ("met", 4096, 4, 8192, 8, True),
        ("mem-missed", 4096, 4, 2048, 8, False),
        ("cpu-missed", 4096, 4, 8192, 2, False),
        ("zero-floor", 0, 0, 0, 0, True),
    ]:
        cases.append(
            case(
                f"resources-{cid}",
                "candidate_resources",
                [
                    integer(mem, bits=64),
                    integer(cpu),
                    integer(gmem, bits=64),
                    integer(gcpu),
                ],
                boolean(expected),
            )
        )
    # Versions: envelope semantics over triples.
    for cid, env, b, g, expected in [
        ("any", "Any", (0, 2, 0), (9, 9, 9), True),
        ("exact-same", "Exact", (0, 2, 0), (0, 2, 0), True),
        ("exact-diff", "Exact", (0, 2, 0), (0, 2, 1), False),
        ("atleast-after", "AtLeast", (0, 2, 0), (0, 3, 0), True),
        ("atleast-before", "AtLeast", (0, 3, 0), (0, 2, 0), False),
        ("compat-same-major", "CompatibleMinor", (1, 2, 0), (1, 5, 0), True),
        ("compat-diff-major", "CompatibleMinor", (1, 2, 0), (2, 0, 0), False),
        ("compat-before", "CompatibleMinor", (1, 5, 0), (1, 2, 0), False),
    ]:
        cases.append(
            case(
                f"version-{cid}",
                "candidate_version",
                [
                    finite("Envelope", env, ENVELOPE[env]),
                    integer(b[0]),
                    integer(b[1]),
                    integer(b[2]),
                    integer(g[0]),
                    integer(g[1]),
                    integer(g[2]),
                ],
                boolean(expected),
            )
        )
    # Optional-feature flags.
    for cid, require, got, expected in [
        ("unrequired-absent", False, False, True),
        ("unrequired-present", False, True, True),
        ("required-present", True, True, True),
        ("required-absent", True, False, False),
    ]:
        cases.append(
            case(f"flag-{cid}", "candidate_flag", [boolean(require), boolean(got)], boolean(expected))
        )
    path = os.path.join(OUT, "platform-capability-corpus.json")
    document = {"schema_version": "0.1", "name": "platform-capability", "cases": cases}
    with open(path, "w") as handle:
        json.dump(document, handle, indent=1)
        handle.write("\n")
    print(f"wrote {path}: {len(cases)} cases")


if __name__ == "__main__":
    main()
