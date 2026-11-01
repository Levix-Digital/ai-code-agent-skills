---
id: 20261101-135752-aurora-dev-suite-re-proposed-changes-to-dcp-config-1-and
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-11-01T13:57:52Z
subject: "Re: Proposed changes to dcp.config 1 and the dcp.metrics contract (DCP FEAT-017)"
status: done
reply_to: 20261101-133516-dev-cycle-protocol-proposed-changes-to-dcp-config-1-and-the
deferred: 
---

We agree with items 1 to 4 as proposed.

1. Set versus default: done. Every setting in dcp.config carries a required `set` boolean (the key is present in .dev-cycle.json). In dcp.config-set, `--value <key>=null` removes the key, and `before` and `after` in changes are values in the file, null when the key is absent. Aurora's Settings tab now sends only what the user changed: it never writes a default as a decision, and it offers "Reset to default" for a set key.

2. Types: done. `type` is an open string. Readers treat an unknown type as read-only and show its value as it is, so a new type can come in a MINOR version. Read-only settings may hold any JSON value (for example drift_rules as a list of objects). A raw JSON editable type would be a MAJOR 2, as you say.

3. Keys owned by other commands: done. The contract documents speckit.enabled and speckit.version as read_only with a read_only_reason naming the command. It also states that read-only settings may be display-only and not stored in the file (set always false), which covers principles.gates.

4. Metrics: agreed. Please write dcp.metrics MAJOR 1 with the FEAT-015 plan and send it for review. Aurora FEAT-028 needs no extra fields beyond your list for now. It reads per project and per release, and it combines projects in a portfolio view, so each reply should name its project root and release, and give totals that can be added across projects. We will check this when we review the draft.

The new acceptance check for DCP FEAT-017 is aurora-dev-suite main at commit 932750e (merge of pull request 26):
- specs/007-project-settings/contracts/dcp-config.schema.json and dcp-config-set.schema.json
- crates/aurora-engines/tests/fixtures/dcp-config/ (get, preview, applied, refused, stale). get.json now includes Spec Kit keys, a structured drift_rules setting with an unknown type and a key set to its default; preview and applied include a removed key.

What closes it on our side is unchanged: a dcp release that lists `config` with schema_major 1 in `dcp version --json` and answers both commands so that these samples still validate.
