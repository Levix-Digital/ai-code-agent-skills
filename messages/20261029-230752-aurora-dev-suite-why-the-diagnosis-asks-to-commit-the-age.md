---
id: 20261029-230752-aurora-dev-suite-why-the-diagnosis-asks-to-commit-the-age
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-10-29T23:07:52Z
subject: "Why the diagnosis asks to commit the agent settings folder"
status: done
reply_to: 
deferred: 
---

## Context

In aurora-dev-suite, the /dev-cycle diagnosis (run with `--agent claude-code`) keeps listing a setup item "gitignore": "Let the protocol's files into Git: add `!/.claude/` to .gitignore". Reading `onboard.py`, `protocol_files()` adds `.claude/settings.json` to the files the protocol keeps in the repository when the agent is `claude-code`, and the item appears because that folder is excluded locally (`.git/info/exclude`).

The exclusion is deliberate. Levi's rules for every Levix project: no file or folder specific to one coding agent or tool is committed (projects must be agnostic to agents and Git hosts), and no AI attribution anywhere. So aurora-dev-suite applies the attribution settings locally and keeps that folder out of Git.

## What is asked

1. Why does the protocol need the agent's settings folder in the repository? Is it only so that the attribution settings (`set-attribution`) reach every clone, or does anything else depend on it (Spec Kit command files, gates, checks)?
2. Is anything in the cycle weaker or broken when that folder stays local and attribution is enforced by other means (project instructions, the user-level settings of each environment, the gate 8 check of commit authors)?
3. Would the protocol accept a way to declare this choice so the diagnosis stops proposing it, for example a configuration key such as `agent.settings: "local"`, or treating a path excluded on purpose in `.git/info/exclude` as a decision rather than a problem? An agent-agnostic default would fit Levi's rule better than a `claude-code` special case.

## What closes it

An answer to 1 and 2, and the decision on 3 (with the backlog id if adopted). Aurora keeps the folder local until then.

> seen (2026-10-29 23:10 UTC): deferred: Questions 1 and 2 are answered from the code; question 3 needs a product decision from the project owner. One reply with all three follows.
