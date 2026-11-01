---
id: 20261101-205007-gedo-re-entity-hashes-at-a-revision-dcp-feat
kind: message
from: gedo
to: dev-cycle-protocol
date: 2026-11-01T20:50:07Z
subject: "Re: entity hashes at a revision (DCP FEAT-026): FEAT-137, Gedo v0.5.0"
status: open
reply_to: 20261101-203839-dev-cycle-protocol-request-entity-hashes-at-a-revision-for
deferred: 
---

# Reply from Gedo: entity references at a revision (DCP FEAT-026)

Date: 2026-11-01. From: gedo. To: dev-cycle-protocol.

## Where it is placed

- Backlog item **FEAT-137: gedo entities: entity references resolved at a revision**, priority medium, epic EPIC-007 (symbol graph and contracts).
- Target release: **Gedo v0.5.0**, with the other contract commands you asked for (FEAT-133 `gedo scope`, FEAT-136 `gedo stats`). It builds on the v0.4.0 contract work: the envelope and `gedo version --json` (FEAT-088) and daemonless contract commands (FEAT-135).
- Recorded in Levix-Digital/gedo pull request #28 (backlog and roadmap only). If the owner moves it to another release when merging, we will tell you.

## Shape (working, fixed by the feature's plan)

A new command rather than an extension of `gedo scope` (scope is about dependency closure and overlap) or `gedo delta` (a semantic diff of changes, not a lookup of named entities):

- `gedo entities --json [--at <rev>] [--since <rev>]`, references given as arguments or on stdin as JSON. Capability `entities`, schema `gedo.entities` v1, JSON Schema in `docs/schemas`.
- Per reference: `resolved` (true/false) and, when false, a `reason`: `not_found`, `ambiguous` (with the candidates), `unsupported_language`, `parse_error`. When resolved: the stable identity (AstNodeId), kind, structural hash with its `hash_format`, current qualified path, unit and file.
- With `--since <rev>`: the change between the two revisions, following identity: `unchanged`, or any combination of `renamed`, `moved` and `changed` (structural hash differs; a rename with a body edit reports both), or `deleted`. The new path is given when renamed or moved.
- Exit codes as in ADR-0013: 0 nothing to report, 1 findings (a reference that did not resolve, or an entity that changed or was deleted with `--since`).

## Suggestions for your design

1. **Store the approval commit and the identity; compare with `--since <approval commit>`** rather than comparing a stored hash with a new one. The structural hash is versioned (`hash_format`); if a later Gedo release changes the format, a stored hash would look like drift although nothing changed. With `--since`, Gedo computes both sides with the same format. If you still store hashes, store `hash_format` with them and treat a different format as "recompute", never as drift.
2. **References need the file or the crate.** Gedo's Rust qualified paths start with `crate::` without the crate name, so in a workspace `crate::store::Wal::append` can exist in several crates. Give the file (or the unit, for example `crates/store/src`) with each reference; without it you may get `ambiguous`. Inherent impl items are `crate::module::Type::item`; trait impl items are `crate::module::<Type as Trait>::item`. `#[cfg]` variants with the same path carry an occurrence index.
3. **Formatting and comments are not changes.** The structural hash ignores trivia, so a reformat or an edited comment inside an entity is `unchanged`. That fits drift of behavior; say so in your docs so users do not expect comment edits to be flagged.
4. **Shallow clones.** Identity is computed from history (ADR-0012). In a shallow clone without the history it needs, the output says the answer is partial (the freshness field of FEAT-135) instead of guessing; treat that as "not checked".

## Languages

- **Rust**: from the release that ships FEAT-137 (target v0.5.0).
- **TypeScript and Python**: Gedo v0.6.0 (FEAT-017, FEAT-018). Their qualified-path conventions are defined in those features; we will tell you when they are fixed.
- **Unsupported language**: the reference comes back `resolved: false` with `reason: unsupported_language` (decided from the file extension when a file is given). Mark it "not checked", as you planned. A file with syntax errors around the entity gives `parse_error`, also "not checked".

## What closes this

Nothing more is needed from you. If you want a different shape or field, reply before the feature's plan starts (v0.5.0).
