#!/usr/bin/env python3
"""Generate human-readable RFC conformance documentation from the canonical
ledger (rfcs/conformance-ledger.json). Never edit the generated file by hand:
fix the ledger and re-run this script.

Usage: python3 scripts/gen_rfc_ledger_docs.py
"""

import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEDGER = os.path.join(ROOT, "rfcs", "conformance-ledger.json")
OUTPUT = os.path.join(ROOT, "docs", "rfc-conformance.md")


def criterion_line(criterion):
    mark = {"satisfied": "[x]", "partial": "[~]", "unsatisfied": "[ ]"}[criterion["state"]]
    line = f"  - {mark} {criterion['id']}: {criterion['text']}"
    if criterion["evidence"]:
        line += " — evidence: " + ", ".join(f"`{path}`" for path in criterion["evidence"])
    return line


def entry_block(entry):
    lines = [
        f"### RFC {entry['number']} — {entry['title']}",
        "",
        f"- Design: **{entry['design_status']}**; implementation: **{entry['implementation_status']}** "
        f"({entry['normativity']}; confidence {entry['evidence_confidence']}).",
        f"- Scope: {entry['implementation_scope']}",
    ]
    if entry["profiles"]:
        lines.append(f"- Profiles: {', '.join(entry['profiles'])}.")
    if entry["stdlib"]:
        lines.append(f"- Stdlib: {', '.join(entry['stdlib'])}.")
    if entry["stages"]:
        lines.append(f"- Stages: {', '.join(entry['stages'])}.")
    if entry["dependencies"]:
        lines.append(f"- Depends on: {', '.join('RFC ' + dep for dep in entry['dependencies'])}.")
    if entry["dependents"]:
        lines.append(f"- Required by: {', '.join('RFC ' + dep for dep in entry['dependents'])}.")
    if entry["tests"]:
        lines.append(f"- Tests: {', '.join(f'`{path}`' for path in entry['tests'])}.")
    lines.append("- Acceptance criteria:")
    lines.extend(criterion_line(criterion) for criterion in entry["acceptance_criteria"])
    if entry["gaps"]:
        lines.append("- Known gaps:")
        lines.extend(f"  - {gap}" for gap in entry["gaps"])
    if entry["pressure_sources"]:
        lines.append(f"- Pressure sources: {', '.join(entry['pressure_sources'])}.")
    if entry["notes"]:
        lines.append(f"- Note: {entry['notes']}")
    lines.append("")
    return lines


def main():
    with open(LEDGER, encoding="utf-8") as handle:
        ledger = json.load(handle)
    entries = ledger["entries"]
    total = len(entries)
    satisfied = sum(
        1
        for entry in entries
        for criterion in entry["acceptance_criteria"]
        if criterion["state"] == "satisfied"
    )
    partial = sum(
        1
        for entry in entries
        for criterion in entry["acceptance_criteria"]
        if criterion["state"] == "partial"
    )
    missing = sum(
        1
        for entry in entries
        for criterion in entry["acceptance_criteria"]
        if criterion["state"] == "unsatisfied"
    )
    lines = [
        "# RFC Conformance",
        "",
        "> Generated from `rfcs/conformance-ledger.json` by",
        "> `scripts/gen_rfc_ledger_docs.py`. Do not edit by hand.",
        "",
        f"Ledger revision `{ledger['revision']}` covering {total} RFCs: "
        f"{satisfied} satisfied, {partial} partially satisfied, {missing} unsatisfied criteria.",
        "",
        "## Reading this document",
        "",
        "- **Design status** (`DRAFT`, `PROPOSED`, `ACCEPTED`, `STABLE`, `SUPERSEDED`)",
        "  tracks the maturity of the RFC text.",
        "- **Implementation status** (`NONE`, `SUBSTRATE`, `PARTIAL`,",
        "  `BOUNDED_IMPLEMENTATION`, `IMPLEMENTED_EXPERIMENTALLY`, `IMPLEMENTED`)",
        "  tracks what the repository can actually do, with evidence per criterion.",
        "- The two axes never collapse: a `DRAFT` design can have a bounded",
        "  implementation, and an `ACCEPTED` design can remain unimplemented.",
        "",
        "## Status overview",
        "",
        "| RFC | Title | Design | Implementation |",
        "| --- | ----- | ------ | -------------- |",
    ]
    for entry in entries:
        lines.append(
            f"| {entry['number']} | {entry['title']} | {entry['design_status']} | "
            f"{entry['implementation_status']} |"
        )
    lines += ["", "## Entries", ""]
    for entry in entries:
        lines.extend(entry_block(entry))
    with open(OUTPUT, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    print(f"wrote {OUTPUT} ({total} entries)")


if __name__ == "__main__":
    main()
