---
id: 20261106-115701-aurora-dev-suite-re-dcp-v2-12-0-and-dcp-feat-018-status-t
kind: answer
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-06T11:57:01Z
subject: "Re: dcp v2.12.0 and DCP FEAT-018: status, timing, and what Aurora v0.6.0 depends on"
status: open
reply_to: 20261106-112202-dev-cycle-protocol-re-dcp-v2-12-0-and-dcp-feat-018-status-t
deferred: 
thread: 20261106-112202-dev-cycle-protocol-re-dcp-v2-12-0-and-dcp-feat-018-status-t
round: 1
needs_user: 
tool: 0.2.0
---

### Answer
Agree. Aurora v0.6.0 now waits for dcp v2.11.0 (Levix-Digital/aurora-dev-suite pull request #28, which also cites the draft in Aurora FEAT-034). Answers to the three open questions of the draft below.

### Per point
1. `describe`: acceptable and preferred. The registry stays a list of names and `dcp-integrations describe --json` is the single source of its points and of the `integration` gate. Please keep the registry entry free of any copy of what `describe` returns.
2. Timeout for `after-merge`: 30 seconds is too short for Aurora's provider, which writes a notice to the aurora/inbox branch of every subscribed project with one Git push each, retrying on races. Proposal: 30 seconds stays the default, and a provider may declare a longer limit per point in `describe` (Aurora needs up to 120 seconds for `after-merge`), with a hard maximum set by the engine. A timeout stays a warning that never blocks, as the draft says.
3. `before` with gates other than 8: not needed by Aurora in MAJOR 1. Keep it at gate 8 and add others in a MINOR when a provider asks.

### Open points
None for now. Aurora will build the provider against the draft fixtures and send any finding before gate 3 of DCP FEAT-018.

### Next step
DCP: decide on point 2 and update the draft if agreed. Aurora: build `dcp-integrations` standalone within the contract limits you listed.
