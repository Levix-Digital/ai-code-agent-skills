"""
New Subsystem Utility - architecture-docs Skill
Creates a living documentation page for a subsystem (docs/architecture/<name>.md)
from the standard template and refreshes the index.
"""
import argparse
import os
import sys

from _docs import Doc, Project, ascii_slug, setup_stdout, write_doc
from build_index import build_index

BODY_TEMPLATE = """# {title}

{summary}

## Responsibilities

What this subsystem owns, and what it explicitly does not.

## Structure

Main modules and types, with paths. A diagram when it helps:

```mermaid
flowchart LR
    A[Caller] --> B[{title}]
```

## Behavior

How it works today: main flows, data formats, configuration, defaults, error handling.

## Interfaces

What other subsystems and surfaces (CLI, TUI, API, MCP) call, and what this subsystem calls.

## Decisions

Link the ADRs that shaped this subsystem, one line each on what they decided.
"""


if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Create a subsystem page in docs/architecture")
    parser.add_argument("--name", type=str, required=True, help="Short page name (e.g. rag, ingestion, tui)")
    parser.add_argument("--title", type=str, required=True, help="Page title (e.g. Retrieval-Augmented Generation)")
    parser.add_argument("--summary", type=str, required=True, help="One sentence on what the subsystem does")
    parser.add_argument("--code", type=str, default="", help="Comma-separated source paths this page documents")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current; the Git root is used)")
    args = parser.parse_args()

    project = Project(args.dir)
    path = os.path.join(project.architecture_dir, f"{ascii_slug(args.name)}.md")
    if os.path.exists(path):
        print(f"❌ {project.rel(path)} already exists.", file=sys.stderr)
        sys.exit(1)
    meta = {"title": args.title, "summary": args.summary,
            "code": [c.strip() for c in args.code.split(",") if c.strip()]}
    write_doc(Doc(path, meta, BODY_TEMPLATE.format(title=args.title, summary=args.summary), True))
    build_index(project)
    print(f"✅ Subsystem page created: {project.rel(path)}")
