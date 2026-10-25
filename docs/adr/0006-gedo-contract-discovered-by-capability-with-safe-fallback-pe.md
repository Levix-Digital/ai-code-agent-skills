---
id: 6
title: Gedo contract discovered by capability with safe fallback per check
status: accepted
date: '2026-10-25'
version: v3.1.0
subsystems: [cycle, quality-release]
supersedes: []
superseded_by: []
amends: [4]
amended_by: []
---

# ADR-0006: Gedo contract discovered by capability with safe fallback per check

## Context

ADR-0004 integrates the protocol with Gedo only through Gedo's command line with `--json`, but leaves open how the protocol knows what an installed Gedo can do, what happens when a command is missing or its output changes, and how the protocol's plans and milestones reach Gedo's events and streams. Gedo's ADR-0013 (Levix-Digital/gedo, accepted on 2026-10-25) now defines the other side: every `--json` output carries `schema` and `schema_version`, `gedo version --json` lists capabilities with the schema major of each, exit codes are stable (0 ok, 1 findings, 2 usage, above 2 failure), and events inherit the author and opaque labels of their stream.

## Decision

- **Commands used.** `gedo version`, `doctor`, `radar`, `delta`, `trace` and `check contracts` (Gedo v0.4.0) for FEAT-004; `gedo scope` and `gedo stream new/list/publish/discard` with `--agent` and `--label` (Gedo v0.5.0) for FEAT-008 and FEAT-003. Nothing else; `graft`, `publish` and the merge driver work through plain Git and Gedo's hooks.
- **Discovery by capability.** Each script reads `gedo version --json` once per run and uses a command only when its capability is listed with a schema major the protocol supports. The protocol never compares Gedo version numbers.
- **Safe fallback per check.** Gedo not installed, a capability missing or an unknown major: that one check runs as if `gedo.enabled` were false, the diagnosis says so, and the case is recorded with `gedo-feedback` as `interface`. Without the stream and scope capabilities the cycle runs in sequence (ADR-0005).
- **Labels.** The protocol's label keys use the `dc.` prefix: `dc.plan`, `dc.milestone`, `dc.role`, `dc.gate`. Agent ids are `<agent>:<session>` (for example `claude-code:<session id>`). The protocol sets them on the stream it opens, or through `GEDO_AGENT` and `GEDO_LABELS` when it works outside a stream.
- **Ownership.** Gedo detects and stores; the protocol decides and orchestrates: which milestones run together, the build lock for test commands, what each finding does to a gate. The protocol never reads `refs/gedo/*` directly.
- **Contract tests.** The protocol keeps recorded `--json` outputs of a released Gedo as test fixtures and tests its parsers against them; a new Gedo schema major means new fixtures and a protocol release.

## Alternatives Considered

- **Check Gedo's version number**: simpler, but breaks when a capability moves between Gedo releases and ties the protocol to Gedo's release train.
- **Fail the cycle when Gedo misbehaves**: surfaces problems loudly, but blocks work on a tool that is still maturing; recording feedback keeps the cycle moving and Gedo's backlog informed.
- **Unprefixed label keys** (`plan`, `milestone`): shorter, but can collide with labels of other tools on the same events.

## Consequences

- FEAT-004 ships in v3.1.0 once Gedo v0.4.0 is released; FEAT-003 and FEAT-008 ship in v3.2.0 once Gedo v0.5.0 is released.
- Each Gedo-backed script needs a small capability reader and recorded fixtures; tests run without Gedo installed.
- Verify by running the scripts with Gedo absent, with an old Gedo and with the current one: the first two must give the same result as `gedo.enabled: false` plus a diagnosis note.
