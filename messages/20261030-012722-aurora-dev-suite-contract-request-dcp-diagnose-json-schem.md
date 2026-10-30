---
id: 20261030-012722-aurora-dev-suite-contract-request-dcp-diagnose-json-schem
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-10-30T01:27:22Z
subject: "Contract request: dcp diagnose --json (schema dcp.diagnose 1.x) for Aurora v0.2.0"
status: seen
reply_to: 
deferred: 
---

Context
Aurora Dev Suite v0.2.0 ("Portfolio on the dcp contract") reads each registered project's development cycle state through the dcp binary only (Aurora ADR-0003, ADR-0006: no Python, no protocol files, no stand-in). Aurora's adapter for it (Aurora FEAT-016) is built and tested against sample replies, and it lights up when a dcp release answers the contract below. As of 2026-10-30 the protocol's backlog has no item for it (DCP FEAT-017 covers config and metrics, DCP FEAT-018 the extension point), and the Rust engine has neither `diagnose` nor `version --json` yet.

What is asked
1. `dcp version --json`: the version reply Aurora already reads (schema `dcp.version`, schema_version 1.x, fields `version` and `capabilities: [{name, schema_major}]`), listing the capability `diagnose` with schema_major 1 once item 2 exists.
2. `dcp diagnose --json --dir <root>`: exit code 0 and one JSON reply on standard output; any other exit code is a failure explained on standard error. Warnings may go to standard error with exit 0.
3. The reply follows schema `dcp.diagnose`, schema_version 1.x (MINOR adds optional fields only, MAJOR breaks; readers ignore unknown fields). The full JSON Schema is in the Aurora repository at specs/002-dcp-adapter/contracts/dcp-diagnose.schema.json (branch feat/002-dcp-adapter, main once merged). Required fields:
   - schema, schema_version, root (absolute repository root the reply is about), summary (one line)
   - next: {ask, recommended (option key or null), options: [{key, title, reason?}]} with today's diagnosis keys (plan:<id>, pr:<number>, rc:<run>, bugs, triage, release, request)
   - release: {version, name, open_items: [item]} or null (the current roadmap release)
   - plans: [{id, title, status, milestones_done, milestones_total}] (active or in_progress)
   - release_candidate: {run, version, not_run, failed, released} or null (latest RC run)
   - pull_requests: [{number, title, summary, needs_action, threads_for_user}] (empty plus a finding when the platform cannot be read)
   - bugs: {blocking: [item], untriaged: [item + reason]}
   - findings: [string] (what could not be read and why; never blocking for the reader)
   - item = {id, rank (priority or severity, or null), title}
   It is the data today's `onboard.py diagnose --json` already collects in `state`, as typed fields instead of preformatted strings ("FEAT-016 (high) dcp adapter"). Setup-only data (config problems, ignored files, Spec Kit, agent attribution) stays out of version 1 and can come later as optional fields.

What closes this request
An item in the protocol's backlog and roadmap that delivers items 1 to 3 (reply with its id and target release), or a counter-proposal for the shape; Aurora will adopt a counter-proposal in a MINOR change of its adapter before either side ships.
