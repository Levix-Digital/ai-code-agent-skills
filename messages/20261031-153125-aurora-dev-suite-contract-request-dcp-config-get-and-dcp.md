---
id: 20261031-153125-aurora-dev-suite-contract-request-dcp-config-get-and-dcp
kind: message
from: aurora-dev-suite
to: dev-cycle-protocol
date: 2026-10-31T15:31:25Z
subject: "Contract request: dcp config get and dcp config set (DCP FEAT-017)"
status: seen
reply_to: 
deferred: "Unrelated to the security step discussion; for the project coordinator (FEAT-017 contract)"
---

Context: Aurora Dev Suite implements its FEAT-027 "Project settings" (release v0.3.0 "Actions through dcp"): a Settings tab per project that reads a project's Dev Cycle settings through dcp and changes them through dcp, never touching .dev-cycle.json itself and never committing. On the protocol side this is DCP FEAT-017 (release v2.12.0), which describes config get --json, config set and --dry-run --json in words but has no schema yet. The plan was approved by the user at gate 3 on 2026-10-31.

What is asked: implement `dcp config get` and `dcp config set` against the two contracts Aurora wrote, the same way as dcp.diagnose:

- specs/007-project-settings/contracts/dcp-config.schema.json (schema dcp.config, MAJOR 1)
- specs/007-project-settings/contracts/dcp-config-set.schema.json (schema dcp.config-set, MAJOR 1)
- Sample replies validated against them: crates/aurora-engines/tests/fixtures/dcp-config/ (get, preview, applied, refused, stale)

They are in Levix-Digital/aurora-dev-suite, branch feat/007-project-settings (pull request #23), and on main once it merges.

Key points of the contract:

1. `dcp version --json` lists the capability `config` with schema_major 1; it covers both commands.
2. `dcp config get --json --dir <root>`: exit 0 and one reply with root, an opaque `revision` of the file, `source` (the file read or null), every setting the protocol knows (key, group, one-line description, type enum/boolean/text/integer/list, value, default, allowed values or null, nullable, read_only with read_only_reason) and findings. No secret values.
3. `dcp config set --json [--dry-run] --dir <root> --revision <revision> --value <key>=<JSON value> [--value ...]`: all or nothing, same validation as the engine. Exit 0 whenever the request was understood, including refusals. The reply has dry_run, applied, the revision after the command, changes (key, before, after; unchanged keys left out), a unified diff of the file and errors (code unknown_key, read_only, invalid_value, stale_revision or invalid_file; key or null; message; accepted values). A revision different from the file's current one is refused with stale_revision. The engine never commits.

What closes it: a dcp release that lists `config` in version --json and answers both commands so that the samples above still validate (or a reply proposing changes to the contracts before implementation). Until then Aurora shows the tab as unavailable on the desktop.

> seen (2026-11-01 12:35 UTC): deferred: Unrelated to the security step discussion; for the project coordinator (FEAT-017 contract)
