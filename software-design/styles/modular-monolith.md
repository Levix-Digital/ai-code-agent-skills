# Modular Monolith

## Use when
- One deployable application covering several business areas (billing, catalog, accounts, reporting).
- You want clear boundaries and independent evolution of areas without the operational cost of microservices.
- Some modules may later become separate services.

## Structure
- **Modules by business capability**, each with its own folder or package containing its domain, application logic and persistence.
- **Public interface per module**: a small facade or API (functions, service interfaces, events) that other modules use. Everything else is internal.
- **Shared kernel**: only truly shared, stable concepts (IDs, money, time). Keep it small.
- **Integration between modules** through the public interface or domain events, never through another module's tables or internal types.

## Rules
- A module never imports another module's internals; enforce it with visibility (`pub(crate)`, `internal`, package-private) and, where possible, with `forbidden_core_imports`-style checks or architecture tests.
- Each module owns its data: its own tables or schema. Cross-module reads go through the owner's interface.
- Circular dependencies between modules are not allowed; if two modules need each other, extract the shared concept or invert one dependency with an event.
- Each module can be tested on its own with the others replaced by their public interfaces.

## Where things go
| Piece | Location |
|---|---|
| A rule about invoices | `billing` module, internal |
| Another module needs an invoice total | `billing`'s public interface |
| "When an account is closed, cancel subscriptions" | Event published by `accounts`, handled by `billing` |
| Money, IDs, clock | Shared kernel |

## Common mistakes
- A "common" or "shared" module that grows into a second monolith everything depends on.
- Joins across modules' tables in one query "for performance" without an agreed read model.
- Modules organized by technical layer (`controllers`, `services`, `repositories`) instead of by business capability.
- Public interfaces that expose internal entities, so every change ripples across modules.
