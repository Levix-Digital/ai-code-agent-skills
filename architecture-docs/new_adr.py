"""
New ADR Utility - architecture-docs Skill
Creates the next Architecture Decision Record (docs/adr/NNNN-<slug>.md) from the
standard template, records relations in both directions (--supersedes / --amends)
and refreshes the index. Accepted ADRs are never rewritten afterwards: only their
status and relation fields change.
"""
import argparse
import sys

from _docs import ADR_FIELD_ORDER, ADR_STATUSES, Doc, Project, adr_label, as_id_list, setup_stdout, today, write_doc
from build_index import build_index

BODY_TEMPLATE = """# {label}: {title}

## Context

What problem or force made this decision necessary? Facts only: current behavior, constraints, measurements.

## Decision

What was decided, stated precisely (components, files, interfaces, defaults).

## Alternatives Considered

- **Alternative A**: why it was not chosen.

## Consequences

What becomes easier or harder, risks, follow-up work, and how to verify the decision holds.
"""


def new_adr(project: Project, title: str, status: str, version: str, subsystems: list,
            supersedes: list, amends: list) -> str:
    adrs = project.adr_by_id()
    for target in supersedes + amends:
        if target not in adrs:
            print(f"❌ {adr_label(target)} not found in {project.rel(project.adr_dir)}.", file=sys.stderr)
            sys.exit(1)

    adr_id = project.next_adr_id()
    label = adr_label(adr_id)
    meta = {
        "id": adr_id, "title": title, "status": status, "date": today(), "version": version,
        "subsystems": subsystems, "supersedes": supersedes, "superseded_by": [],
        "amends": amends, "amended_by": [],
    }
    doc = Doc(project.adr_path(adr_id, title), meta, BODY_TEMPLATE.format(label=label, title=title), True)
    write_doc(doc, ADR_FIELD_ORDER)

    for field, back, target_ids in (("supersedes", "superseded_by", supersedes), ("amends", "amended_by", amends)):
        for target in target_ids:
            old = adrs[target]
            old.meta[back] = sorted(set(as_id_list(old.meta.get(back)) + [adr_id]))
            if field == "supersedes":
                old.meta["status"] = "superseded"
            write_doc(old, ADR_FIELD_ORDER)
            print(f"🔗 {adr_label(target)}: {back} += {label}" + (" (status: superseded)" if field == "supersedes" else ""))

    build_index(project)
    print(f"✅ {label} created: {project.rel(doc.path)}")
    print("   👉 Fill in Context, Decision, Alternatives Considered and Consequences, link the ADR from the")
    print("      affected subsystem pages, run check_docs.py and commit.")
    return doc.path


if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Create the next Architecture Decision Record")
    parser.add_argument("--title", type=str, required=True, help="Decision title, without version numbers")
    parser.add_argument("--status", type=str, default="accepted", choices=ADR_STATUSES[:2], help="Initial status")
    parser.add_argument("--version", type=str, default=None, help="Version that ships the decision (e.g. v0.46.0)")
    parser.add_argument("--subsystem", type=str, default="", help="Comma-separated subsystem page names (e.g. rag,onnx)")
    parser.add_argument("--supersedes", type=str, default="", help="Comma-separated ADR numbers this one replaces entirely")
    parser.add_argument("--amends", type=str, default="", help="Comma-separated ADR numbers this one corrects in part")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current; the Git root is used)")
    args = parser.parse_args()

    new_adr(
        Project(args.dir), args.title, args.status, args.version,
        [s.strip() for s in args.subsystem.split(",") if s.strip()],
        as_id_list(args.supersedes.split(",")), as_id_list(args.amends.split(",")),
    )
