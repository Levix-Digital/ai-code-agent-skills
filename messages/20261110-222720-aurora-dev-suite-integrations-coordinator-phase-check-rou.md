---
id: 20261110-222720-aurora-dev-suite-integrations-coordinator-phase-check-rou
kind: proposal
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-10T22:27:20.513Z
subject: "Integrations coordinator: phase check, round table, hold and verify in the Dev Cycle"
status: seen
reply_to: 
deferred: 
thread: 20261110-222720-aurora-dev-suite-integrations-coordinator-phase-check-rou
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Take into the Dev Cycle Protocol backlog the cycle-side parts of the integrations coordinator decided by the user on 2026-11-10, or answer with a counter-proposal for any of them.

### Context
The user decided on 2026-11-10 how projects settle integration changes between them: a phase check, a round table chaired by a neutral coordinator, approval by every involved project, and an integration hold. Aurora recorded it in aurora-dev-suite ADR-0015 (pull request #45, stacked on #44 with ADR-0014). Aurora becomes a self-hosted web application governed by the Levix Admin Hub, which signs approvals (Hub FEAT-047, FEAT-048, FEAT-049 in Hub v0.4.0, agreed 2026-11-10). This builds on DCP FEAT-018 (extension point, dcp v2.11.0) and Aurora FEAT-036 and FEAT-037. The Dev Cycle Protocol must keep working with neither Aurora nor a Hub.

### Proposal
1. Phase check. At every phase, a check reads the project's content up to that phase and decides whether it may touch another project's contracts; it names the projects involved and the projects only informed.
2. Coordinator and round table. When the check finds a possible adjustment, a neutral coordinator runs the table (`dcp integrations table <id>`): opening to every involved project; independent initial answers; "not affected" leaves with the contracts checked and is called back if a counterproposal touches them; each project reviews every other answer (agree or counterproposal); unanimous agreement closes; otherwise consolidated counterproposals, three rounds at most; confirmation of the final text (not a round); impasse record for every involved project's approvers. Seats and the coordinator are calls to a configured command, so any agent can run them; each seat reads only its own project's records and cites them. Messages, the five templates (table-opening, table-answer, table-review, table-proposal, table-impasse) and storage are provided by dcp-integrations (Aurora FEAT-048).
3. Integration hold. The cycle stops at a hold until every involved project has approved; the hold is recorded in aurora/inbox, linked to the plan and the branch; an open session watches the inbox and continues on the last approval; a later session resumes from the same phase. A question or objection from an approver starts a new table, three at most per decision.
4. Required check at merge. `dcp integrations verify` fails a pull request whose plan or branch has an open hold without valid approvals; it reads `.aurora/approvers.json` from the target branch and verifies Hub-signed attestations offline. `verify --branches` lists the long-lived branches of the branching model to protect (integration: dev and main; trunk: main; release-branches: main and every release/X.Y). The phase refusing to advance and a pre-push warning are conveniences only.
5. Without Aurora or a Hub. A project with no `.aurora/approvers.json` needs no approval: the developer's acceptance closes the hold. A project that has one never moves without the signed approvals.
6. E-mail fallback. Without a Hub, `dcp mail setup` configures an SMTP account or a mail command once per machine (password in the operating system keychain) and the coordinator notifies approvers at once; without it, the hold names the approvers.

### Constraints
Decided by the user on 2026-11-10: the dynamics in point 2; approval by every involved project; the hold and the required check at merge on every long-lived branch; approvals signed by the Admin Hub; the Dev Cycle Protocol works without Aurora and without a Hub. Everything stays agnostic to coding agents and Git hosts. The only write in other projects' repositories is their aurora/inbox branch through dcp-integrations.

### Closes when
The Dev Cycle Protocol answers with backlog item ids and a target release for points 1 to 6, or with a counter-proposal for any of them.

### Authority
The projects may settle item ids, release placement, command names and the split between dcp and dcp-integrations. Any change to the dynamics, the approval scope, the enforcement at merge or standalone operation goes to the user.
