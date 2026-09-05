#!/usr/bin/env python3
"""Owner-native pressure-test check for the mncs-language family boundary.

Runs the compiler pressure-test target (module imports, including the
P-COMMONS-03 qualified-record coverage the Commons mesh forces) and
writes one mncs.check-result/1 document. Exit 0 always carries the
verdict file; a FAIL verdict is data, never a crash.

Scope is deliberately the pressure target, not the full workspace
suite: ``cargo test --workspace`` plus clippy/fmt stay in this repo's
own CI. The boundary declaration in the caller workflow says exactly
this.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

RESULT_SCHEMA = "mncs.check-result/1"
CHECK_ID = "language-pressure-tests"
PROVIDER = "mncs-language-cargo"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--result-file", required=True)
    parser.add_argument("--revision", default="working-tree")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    completed = subprocess.run(
        ["cargo", "test", "-p", "mncs-compiler", "--test", "module_imports"],
        capture_output=True,
        text=True,
        check=False,
        cwd=repo,
        timeout=1800,
    )
    verdict = "PASS" if completed.returncode == 0 else "FAIL"
    tail = (completed.stdout + completed.stderr)[-800:]
    last = tail.strip().splitlines()[-1] if tail.strip() else "no output"
    result = {
        "schema_version": RESULT_SCHEMA,
        "id": CHECK_ID,
        "provider": PROVIDER,
        "verdict": verdict,
        "summary": f"cargo test -p mncs-compiler --test module_imports exit={completed.returncode}: {last}",
        "subject": {"repository": "mncs-language", "revision": args.revision},
    }
    destination = Path(args.result_file)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"id": CHECK_ID, "verdict": verdict}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
