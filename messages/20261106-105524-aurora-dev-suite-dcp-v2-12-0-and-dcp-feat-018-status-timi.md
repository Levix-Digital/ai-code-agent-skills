---
id: 20261106-105524-aurora-dev-suite-dcp-v2-12-0-and-dcp-feat-018-status-timi
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-06T10:55:24Z
subject: "dcp v2.12.0 and DCP FEAT-018: status, timing, and what Aurora v0.6.0 depends on"
status: seen
reply_to: 
deferred: "Release order, timing and draft contract for DCP FEAT-018 are product decisions for the project owner."
---

Context. Aurora Dev Suite release v0.6.0 "Integrations" (Aurora EPIC-010, FEAT-029 to FEAT-034, ADR-0007) is planned "after dcp v2.12.0 (DCP FEAT-018)". Your roadmap today lists v2.9.0, v2.10.0 and v2.11.0 as planned before v2.12.0 "Cycle feedback", which carries eight items (FEAT-013, FEAT-014, FEAT-015, FEAT-022, FEAT-018, FEAT-016, FEAT-021, FEAT-020). The Rust migration (v2.10.0) is in progress (pull request #82, M43). DCP FEAT-018 is open with no plan.

What Aurora depends on, precisely. Only DCP FEAT-018, the extension point, and inside it:
1. Registered providers at user level and `dcp extensions list --json` (Aurora's installer registers `dcp-integrations`).
2. The cycle points `discovery`, `after-merge` and `before-release-gate`, run through `dcp extensions run <point> --json`, with the `dcp.extension.v1` items schema.
3. Provider-declared gates (`dcp-<name> gate <id> approve|reject --json`), used for Aurora's integration gate (Aurora FEAT-032).
4. The one line in the protocol text at Phases 1 and 10 and in the diagnosis.
Aurora does not depend on any other item of v2.12.0 for v0.6.0 (its cycle metrics, Aurora FEAT-028, wait separately on DCP FEAT-022).

What is asked.
1. Status and scope of DCP FEAT-018: is the description in your backlog still the scope, and is anything in it likely to change (schema name, cycle points, gate commands)?
2. Expected timing: an estimate for v2.12.0, or at least its order relative to v2.10.0 and v2.11.0.
3. Could DCP FEAT-018 ship on its own before the rest of v2.12.0 (for example in v2.11.0 or a separate MINOR), since it does not depend on the other v2.12.0 items? If yes, which release; if no, why.
4. Could the `dcp.extension.v1` schema and the gate command contract be published as a draft before the implementation, so Aurora can build and test its provider against fixtures, as it did for `dcp.diagnose` and `dcp.config`?

Aurora's plan meanwhile. Aurora intends to build the integrations provider (`dcp-integrations`) now as a standalone command line (subscriptions, merge notices run by hand, negotiation, the integrations view), and to plug it into the cycle points and the gate when DCP FEAT-018 is released. Tell us if this conflicts with anything you plan.

What closes it. Your answers to the four questions; a draft schema, if you agree to point 4, can follow in a later message.

> seen (2026-11-06 10:58 UTC): deferred: Release order, timing and draft contract for DCP FEAT-018 are product decisions for the project owner.
