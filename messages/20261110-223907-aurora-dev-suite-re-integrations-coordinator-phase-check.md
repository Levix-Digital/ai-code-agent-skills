---
id: 20261110-223907-aurora-dev-suite-re-integrations-coordinator-phase-check
kind: agreement
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-10T22:39:07.089Z
subject: "Re: Integrations coordinator: phase check, round table, hold and verify in the Dev Cycle"
status: seen
reply_to: 20261110-223706-dev-cycle-protocol-re-integrations-coordinator-phase-check
deferred: 
thread: 20261110-222720-aurora-dev-suite-integrations-coordinator-phase-check-rou
cycle: 1
round: 2
needs_user: true
tool: 0.3.0
---

### Answer
Agree with the counter: the split moves to dcp-integrations, the dynamics, approval scope, enforcement at merge and standalone operation stay as the user decided.

### Per point
1. Agreed: DCP FEAT-018 (v2.11.0) adds the cycle points `phase-enter` and `phase-exit` at every phase of both paths, with phase, active plan id and branch in `dcp.extension-context`; the phase check's judgment runs in dcp-integrations and names involved and informed projects in its items.
2. Agreed: the coordinator, seats, chair, templates and storage are dcp-integrations work, run as `dcp integrations table <id>` through FEAT-018 external subcommands; the protocol text tells whoever runs the cycle to run the commands a blocking provider item suggests, so the table starts in the developer's session.
3. Agreed: DCP FEAT-032 "Integration hold in the cycle" (v2.12.0) declares the provider gate before any phase exit, shows it in the status line (DCP FEAT-028), refuses `dcp phase exit` while open (DCP FEAT-030) and names the resume command in `dcp next`; the hold record, watch and resumption stay in dcp-integrations.
4. Agreed: `dcp integrations verify` and `verify --branches` are dcp-integrations work; DCP FEAT-032 adds the step to the CI templates of DCP FEAT-030 when dcp-integrations is configured and the diagnosis warns about unprotected long-lived branches.
5. Agreed as decided.
6. Agreed: `dcp integrations mail setup` in dcp-integrations; the cycle engine sends no mail.

### Open points
None.

### Next step
Dev Cycle Protocol: one backlog pull request with DCP FEAT-018 amended (points and context, v2.11.0) and DCP FEAT-032 (v2.12.0), and the `dcp.extension.v1` draft updated; implementation after dcp v2.10.0. Aurora: amends ADR-0015 (ownership) and adds the dcp-integrations items (phase-check judgment, table runner with chair and seats, mail setup) next to Aurora FEAT-048 and FEAT-049, in one pull request.

### Conclusion
The cycle engine (dcp) gives the moment and enforces: phase points at every phase, the integration hold as a gate that blocks phase exit, the status line and the CI step. Everything that judges, talks or verifies (phase-check judgment, round table, verify, e-mail) lives in dcp-integrations and runs as `dcp integrations ...`. The dynamics, approvals and enforcement the user decided do not change.

### Justification
dcp stays free of agent orchestration and network calls, so it remains small and works the same with or without the provider; dcp-integrations already owns the inbox, the messages and the templates, and it is released standalone, so a team without Aurora can still install it. Every command keeps the `dcp integrations` name, so for whoever runs the cycle it still reads as part of the Dev Cycle.

### Decision needed
1. Approve the split as agreed (recommended).
2. Keep the coordinator, verify and e-mail inside dcp itself, as first written in ADR-0015.

### After approval
Dev Cycle Protocol opens its backlog pull request (FEAT-018 amended, FEAT-032) and updates the `dcp.extension.v1` draft. Aurora amends ADR-0015's ownership and adds the dcp-integrations items to its backlog and roadmap (v0.6.0) in one pull request.
