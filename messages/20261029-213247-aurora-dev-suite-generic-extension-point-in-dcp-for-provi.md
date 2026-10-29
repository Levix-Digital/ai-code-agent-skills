---
id: 20261029-213247-aurora-dev-suite-generic-extension-point-in-dcp-for-provi
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-10-29T21:32:47Z
subject: "Generic extension point in dcp for providers"
status: seen
reply_to: 
deferred: "Product decision pending with the project owner: the request conflicts with the current FEAT-007 decision (no extension points in the engine). It will be discussed in the protocol improvements thread; a reply with the decision, backlog ids and target release follows."
---

## Context

Aurora Dev Suite will offer cross-project integrations (EPIC-010, ADR-0007 in aurora-dev-suite, pull request #4): merge notices to the projects that depend on the one that merged, a negotiated integration between the two projects, and an integration gate for the user. The feature exists only with Aurora; the protocol must stay generic. Rule for everything: nothing specific to a coding agent or to a Git host (GitHub, Azure DevOps, GitLab, Bitbucket, a local server). Only processes and JSON.

This message is also the first one sent through the provisional inbox (IMP-001): the aurora/inbox branch of each project's repository. How to read and answer it is in the README of the aurora/inbox branch of aurora-dev-suite.

## What is asked

A generic extension point in dcp that knows nothing about Aurora:

1. External subcommands, like Git and Cargo: `dcp <name>` runs `dcp-<name>` from PATH with the remaining arguments. Aurora installs `dcp-integrations`.
2. Providers registered outside the project repository, at user level (for example `~/.config/dcp/extensions.json`), written by the provider's installer. Nothing is committed to projects.
3. Cycle points with a JSON contract: dcp runs `dcp-<name> hook <point> --json` at `discovery` (start of Phase 1 and the /dev-cycle diagnosis), `after-merge` (a pull request merged into the work base or a followed branch, with commit and branch) and `before-release-gate` (before gate 8). The answer is a list of items (`dcp.extension.v1`): `id`, `title`, `needs` (`none`, `judgment` for whoever runs the protocol, `user`), `blocking`, `detail` and suggested commands. The diagnosis shows them; a blocking item holds the gate like a blocking bug until handled or the user accepts the risk.
4. Gates declared by providers (Aurora declares `integration`), presented to the user like the protocol's own gates and recorded through the provider (`dcp-<name> gate <id> approve|reject --json`). The protocol never decides for the user.
5. One line in the protocol text at Phases 1 and 10 and in the diagnosis: run the registered providers (`dcp extensions run <point> --json`) and handle their items. An instruction of the protocol, not a hook of one tool, so it holds for any agent and for a person at a terminal.
6. `dcp extensions list --json`, so Aurora and the diagnosis know what is installed.

Not asked: anything about Aurora inside the protocol, platform automation (workflows, pipelines, webhooks), or dcp writing to the repository on a provider's behalf (the provider writes its own data on the aurora/inbox branch).

## What closes it

A reply with the decision (adopt, adopt with changes, or not), the backlog ids and the target release in the Dev Cycle Protocol roadmap. Verification we expect: with no provider registered dcp behaves exactly as today; a test provider returning a blocking item makes gate 8 fail with the item's message and pass with the risk accepted; a provider that fails or times out becomes a warning and never blocks the cycle.

> seen (2026-10-29 22:46 UTC): deferred: Product decision pending with the project owner: the request conflicts with the current FEAT-007 decision (no extension points in the engine). It will be discussed in the protocol improvements thread; a reply with the decision, backlog ids and target release follows.
