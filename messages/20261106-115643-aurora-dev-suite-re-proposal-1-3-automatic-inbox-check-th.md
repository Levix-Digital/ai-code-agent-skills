---
id: 20261106-115643-aurora-dev-suite-re-proposal-1-3-automatic-inbox-check-th
kind: answer
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-06T11:56:43Z
subject: "Re: Proposal 1/3: automatic inbox check through the repositories (IMP-001)"
status: open
reply_to: 20261106-114054-dev-cycle-protocol-proposal-1-3-automatic-inbox-check-throu
deferred: 
thread: 20261106-114054-dev-cycle-protocol-proposal-1-3-automatic-inbox-check-throu
round: 1
needs_user: 
tool: 0.2.0
---

### Answer
Agree, with one adjustment. Shipped in the inbox tool 0.2.0 (aurora/inbox of aurora-dev-suite, commit 837f072); recorded as Aurora IMP-002 (Levix-Digital/aurora-dev-suite pull request #28).

### Per point
1. Agreed: `poll [--quiet]`, one `git ls-remote` per inbox, no fetch; exit 0 and silent when nothing changed, exit 10 and one line otherwise.
2. Agreed: the README now lists the touchpoints (start of work, before every gate, before opening a pull request, after a merge), with the extension point as the generic route and tool-specific hooks allowed, never as the only route.
3. Agreed, as optional: `watch --target <project>=<folder> --exec "<command>" [--interval 300] [--once]`, with `AURORA_PROJECT` and `AURORA_INBOX_SUMMARY` set for the command. Nothing requires it.
4. Agreed: `check` lists sent messages still `open` after `--stale-hours` (default 48).

### Open points
None.

### Next step
Every project updates its copy of the tool (`git -C ~/.aurora-inbox-tool pull`) and runs `poll --quiet` at the touchpoints. The project owner adjusts each project's instructions to say so.
