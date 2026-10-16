# Hexagonal Architecture (Ports and Adapters)

## Use when
- One domain core serves several interfaces: CLI, terminal UI, desktop GUI, REST API, RPC, MCP, editor extensions.
- Every interface must behave the same on the same data.
- Infrastructure (database, model providers, network) should be replaceable or testable in isolation.

## Structure
- **Core (domain + application services)**: entities, business rules, use cases and the **ports** they need (interfaces/traits for storage, providers, clocks, file systems). No framework, UI, transport or driver code.
- **Driving adapters** (input): CLI, TUI, HTTP handlers, RPC, MCP servers. They translate requests into core calls and core results into output.
- **Driven adapters** (output): database repositories, HTTP clients, model providers, file system access. They implement the core's ports.
- **Composition root**: one place per executable that builds the adapters and wires them into the core.

## Dependency rule
Adapters depend on the core; the core depends on nothing outside itself except plain libraries (serialization, math, date handling). List the adapter and framework modules in `architecture.forbidden_core_imports` so `check_design.py` enforces the rule.

## Surface parity and single source of truth
- Every user action is one core operation (a command or use case). All interfaces dispatch to it; no interface keeps its own rules, filters or shortcuts.
- Results are plain data (messages, actions, state updates) that each interface renders in its own way.
- All interfaces read and write **the same canonical state**. A second database, cache or table for one interface is forbidden.
- Declare the interfaces in `architecture.surfaces` and the canonical store in `architecture.canonical_datastore`; plans state which surfaces a change touches.

## Where things go
| Piece | Location |
|---|---|
| A business rule or validation | Core service or entity |
| A new user command | Core command/use case, then one thin handler per surface |
| Database schema or query | Driven adapter implementing a core repository port |
| A new LLM, payment or storage provider | Driven adapter implementing an existing port |
| Output formatting, colors, layout | Driving adapter |

## Common mistakes
- Business logic in a CLI handler or TUI event loop "because it was quicker".
- The core constructing its own HTTP client, database connection or UI widget.
- Ports shaped like the first adapter's API (leaking driver types into the core).
- A port for something with one implementation and no test need (see "Simplicity First").

## Examples by ecosystem
- **Rust**: `crates/<app>-core` (domain, ports as traits), adapter crates or binaries (`crates/<app>-cli`), storage adapters in their own modules.
- **Python**: `src/<app>/core/` and `services/`; adapters in `cli.py`, `api/`, `mcp/`.
- **TypeScript**: `packages/core`; `apps/cli`, `apps/web`, `apps/desktop`.
- **Go**: `internal/domain`, `internal/app`; `cmd/cli`, `cmd/server`, `internal/adapters/...`.
