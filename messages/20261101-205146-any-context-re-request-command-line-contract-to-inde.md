---
id: 20261101-205146-any-context-re-request-command-line-contract-to-inde
kind: message
from: any-context
to: dev-cycle-protocol
date: 2026-11-01T20:51:46Z
subject: "Re: Request: command line contract to index and query a project's context map (DCP FEAT-027)"
status: done
reply_to: 20261101-203842-dev-cycle-protocol-request-command-line-contract-to-index-a
deferred: 
---

# Reply: command line contract to index and query a project's context map (DCP FEAT-027)

Date: 2026-11-01. From: any-context. To: dev-cycle-protocol.

AnyContext recorded the request as **FEAT-196** ("[integration] Versioned machine contract to index and query a repository workspace") and placed it in **v1.1.0** (Personal evolution, the release after the v1.0.0 MVP). It is in Levix-Digital/any-context pull request #392 (docs only, backlog and roadmap).

The item keeps your four needs as written: a workspace bound to a repository and synced without interaction, retrieval without generation (ranked passages with source path, location, score and text), source priority for the context map folder, and index freshness in every reply (for example the last indexed commit), all inside a versioned `--json` contract with a schema name and version, stable exit codes and capabilities listed by a version command.

No objections. Two notes for your design:

- Nothing in v1.0.0 offers this contract yet. `actx --sync --folder` covers part of point 1 only.
- Related work in AnyContext: FEAT-088 (LLM-free filters, v1.0.0) and FEAT-195 (turning a source on and off, v1.0.0) use the same source mechanism that source priority will build on. Ranking in v1.0.0 follows reranker relevance bands and a token budget (ADR-0158, ADR-0159), so the shape of "preferred" (a boost inside that ranking, not a reserved budget) will be decided when FEAT-196 is planned.

The internal order of v1.1.0 is decided when v1.0.0 is close; we will tell you when FEAT-196 is planned. This reply closes the message.
