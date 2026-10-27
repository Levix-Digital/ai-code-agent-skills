---
id: 1
title: Skills in the open SKILL.md format with Python scripts
status: accepted
date: '2026-10-25'
version: null
subsystems: [cycle, configuration, planning, quality-release, documentation, memory]
supersedes: []
superseded_by: []
amends: []
amended_by: [7]
---

# ADR-0001: Skills in the open SKILL.md format with Python scripts

## Context

The protocol must work with every coding agent a developer uses (Claude Code, Codex, Gemini, Copilot and others), on a local machine and in cloud sessions. Instructions alone are not enough: gates must be checked deterministically, not on the agent's word.

## Decision

Each skill is a folder with a `SKILL.md` in the open format (frontmatter `name` and `description`, Markdown body) and the Python 3.10+ command line scripts it calls. Scripts use the standard library first, take `--dir` and print plain text or `--json`. One checkout is linked into every agent's skills folder (`install.sh`, `install.ps1`) and shipped to Claude Code cloud sessions as a plugin (`.claude-plugin/`).

## Alternatives Considered

- **Agent-specific formats** (Cursor rules, custom GPT instructions): lock the protocol to one agent.
- **A compiled tool** (Rust or Go binary): fast and dependency-free, but harder for agents to read and change, and one more build per platform. Revisited once Gedo has a command line (VISION open question).
- **Instructions only, no scripts**: gates would depend on the agent following the text.

## Consequences

- Any agent can read and run the protocol; tests run with `pytest`.
- Python environment friction remains (missing modules when the agent's Python differs); IMP-003 addresses it with `uv` and a standard-library search fallback.
