"""Fabric bundle entry point for a frozen MNCS backend artifact and corpus."""

from __future__ import annotations

import json
from pathlib import Path
import stat
import subprocess


def main() -> int:
    bundle = Path(__file__).resolve().parent
    executable = bundle / ("mncs.exe" if (bundle / "mncs.exe").is_file() else "mncs")
    if executable.name == "mncs":
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
    completed = subprocess.run(
        [
            str(executable),
            "experiment",
            "execute",
            "backend-artifact.json",
            "corpus.json",
        ],
        cwd=bundle,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if completed.returncode != 0:
        raise SystemExit(completed.stderr or f"mncs exited {completed.returncode}")
    observations = json.loads(completed.stdout)
    (bundle / "result.json").write_text(
        json.dumps(observations, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
