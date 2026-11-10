# Contract draft: dcp extension point (FEAT-018)

Status: **draft**. Nothing here is implemented yet; FEAT-018 is planned for v2.11.0. This draft exists so a provider (Aurora Dev Suite's `dcp-integrations` first) can be built and tested against fixtures before the engine ships. Names, fields and commands may still change until FEAT-018 passes gate 3; any change is announced to the providers that follow this draft. After the release, the contract follows the versioning rules below.

The behavior is the one described in `docs/BACKLOG.md` (FEAT-018). This file fixes the wire format: commands, arguments, JSON documents, exit codes and failure handling.

Files:

- `schema.json`: JSON Schema (draft 2020-12) with one definition per document below.
- `fixtures/`: sample documents, valid against `schema.json`.

## Principles

- The engine knows no provider. A provider is any executable named `dcp-<name>` that answers the commands below; it talks to the engine only through processes, arguments and JSON on standard output.
- A provider only adds items, warnings and its own gates. It never skips, weakens or approves the protocol's gates, and the engine never writes to the project repository on a provider's behalf.
- Suggested commands in an item are shown to whoever runs the protocol, never run by the engine.
- A provider that fails, times out or prints invalid JSON becomes a warning and never blocks the cycle.
- With no provider registered, dcp behaves exactly as without this feature.

## Names

`<name>` matches `^[a-z][a-z0-9-]{0,39}$`. The executable is `dcp-<name>` (`dcp-<name>.exe` on Windows), looked up on `PATH` unless the registry gives an absolute `command`.

`dcp <name> <args...>` runs `dcp-<name> <args...>` with the same standard input, output and exit code (external subcommands, as in Git and Cargo). This works for any `dcp-*` on `PATH`; only **registered** providers run at cycle points.

## Registry (user level)

Written by the provider's installer, never committed to a project. Location:

| Platform | Path |
| --- | --- |
| Linux, cloud sessions | `$XDG_CONFIG_HOME/dcp/extensions.json`, default `~/.config/dcp/extensions.json` |
| macOS | `~/.config/dcp/extensions.json` |
| Windows | `%APPDATA%\dcp\extensions.json` |

`DCP_CONFIG_DIR`, when set, replaces the directory on every platform (used by tests).

Document `dcp.extensions-registry`, see `fixtures/registry.json`:

- `schema`: `"dcp.extensions-registry"`, `schema_version`: `"1.0"`.
- `providers[]`: `name` (required), `command` (optional absolute path), `enabled` (optional, default `true`).

An installer adds or replaces its own entry by `name` and leaves the others untouched. An unreadable registry is a warning, and no provider runs.

## Provider commands

Every command takes `--json`, prints exactly one JSON document on standard output and exits `0` on success. Anything on standard error is shown only in the warning when the call fails. The engine sets these environment variables on every call:

- `DCP_VERSION`: the engine version.
- `DCP_PROJECT_ROOT`: absolute path of the project root.
- `DCP_EXTENSION_CONTRACT`: `1`.

Timeout: 30 seconds per call by default (`DCP_EXTENSION_TIMEOUT` in seconds overrides the default). A provider may declare a longer limit for a `hook` point in `describe` (`timeouts`); the engine caps every limit at 300 seconds. `describe` and `gate` always use the default. On timeout the process is killed and the call becomes a `timeout` warning, which never blocks.

### `dcp-<name> describe --json`

Document `dcp.extension-provider` (`fixtures/describe.json`): `name`, `version` (the provider's own), `contract` (`1`), `points[]` (the cycle points it answers) and `gates[]` (`id`, `title`, `before`: the protocol gate it must be decided before, today only `"8"`, and `detail`), and optionally `timeouts`: an object from point to seconds (integer, 1 to 300), for example `{"after-merge": 120}`. The engine calls it for `dcp extensions list` and before running cycle points; a provider is only called at the points it declares.

### `dcp-<name> hook <point> --json`

`<point>` is one of:

| Point | When |
| --- | --- |
| `discovery` | Start of Phase 1 and the `/dev-cycle` diagnosis |
| `phase-enter` | Every phase of the full and short paths, when it starts (after `dcp phase enter`) |
| `phase-exit` | Every phase of the full and short paths, before it ends (before `dcp phase exit` passes) |
| `after-merge` | A pull request merged into the work base or a followed branch (Phase 10) |
| `before-release-gate` | Before gate 8 |

The engine writes a context document `dcp.extension-context` on standard input (`fixtures/context-after-merge.json`): `point`, `project_root`, `release` (the current release version or `null`), `phase`, `plan` (the active plan as `id` and `branch`, or `null` when there is none) and, for `after-merge` only, `merge` with `commit`, `branch` and `base`.

The provider answers `dcp.extension` (the `dcp.extension.v1` items document) (`fixtures/items-before-release-gate.json`): `provider`, `point` and `items[]`. Each item has:

- `id`: stable for the same finding across runs, unique per provider (`^[A-Za-z0-9._-]{1,80}$`).
- `title`: one line.
- `needs`: `none` (information), `judgment` (whoever runs the protocol decides) or `user` (only the user can decide).
- `blocking`: when `true`, holds the next gate like a blocking bug until the item is handled (no longer returned) or the user accepts the risk.
- `detail`: optional text.
- `commands[]`: optional suggested commands, shown, never run.

An empty `items` is the normal answer when there is nothing to report.

### `dcp-<name> gate <id> status|approve|reject --json [--reason <text>]`

Document `dcp.extension-gate` (`fixtures/gate-approve.json`): `provider`, `gate`, `decision` (`pending`, `approved` or `rejected`), `decided_at` (RFC 3339 or `null`), `reason` (or `null`). The provider records the decision in its own storage. The engine presents provider gates to the user like its own and records only what the user decides; it calls `status` to know whether a provider gate is still open before the protocol gate named in `before`.

`--reason` is required with `reject`.

## Engine commands

### `dcp extensions list --json`

Document `dcp.extensions` (`fixtures/list.json`): `registry` (path) and `providers[]` with `name`, `command` (resolved path or `null`), `enabled`, `status` (`ok`, `missing`, `failed`, `incompatible`), the provider's `version`, `points` and `gates` when `describe` answered, and `warning` (or `null`).

### `dcp extensions run <point> --json`

Runs every registered, enabled provider that declares `<point>`, in registry order. Document `dcp.extensions-run` (`fixtures/run-before-release-gate.json`): `point`, `items[]` (each item above plus `provider`), `blocking` (`true` when any item blocks) and `warnings[]` (`provider`, `kind`: `missing`, `failed`, `timeout`, `invalid_json`, `incompatible`, and `detail`). Exit code `0` even when items block; the gate check decides, not this command. With no provider registered, the answer has empty `items` and `warnings`.

## Versioning

- Every document carries `schema` and `schema_version` (`"MAJOR.MINOR"`, as in `dcp.diagnose` and `dcp.config`). This draft is `1.0`; v1 in `dcp.extension.v1` is the MAJOR.
- Within MAJOR 1, changes are additive only and raise MINOR: new optional fields and new enum values in fields marked open (`points`, `kind`). Readers ignore unknown fields.
- A provider whose `describe` answers a `contract` MAJOR the engine does not support is listed as `incompatible` and not run.
- Removing or renaming a field, or changing its meaning, is a new MAJOR.

## Decisions on the draft

1. Points and gates are declared only by `describe`. The registry stays a list of names and holds no copy of what `describe` returns.
2. A provider may declare a longer timeout per `hook` point in `describe`; 30 seconds stays the default and the engine caps every limit at 300 seconds.
3. `before` accepts only gate `8` in MAJOR 1; other gates may be added in a MINOR when a provider asks for them.
4. Cycle points `phase-enter` and `phase-exit` run at every phase, with the active plan in the context, so a provider can judge at each phase whether the work may touch another project's contracts (Aurora Dev Suite's phase check, agreed 2026-11-10 with Aurora ADR-0015). The judgment is the provider's; the engine only gives the moment. Gates declared before a phase exit come with FEAT-032 (v2.12.0) as a MINOR of this contract.
