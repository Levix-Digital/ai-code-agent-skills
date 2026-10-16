# Layered Architecture

## Use when
- A conventional application with one main interface (a web app, an internal service, a desktop app).
- The team benefits from a familiar, explicit separation more than from swappable adapters.

## Structure
1. **Presentation**: UI, controllers, request/response mapping, input validation of shape (not of business rules).
2. **Application**: use cases that orchestrate a request: load, call the domain, save, emit events. Transactions live here.
3. **Domain**: entities, value objects and business rules. No I/O.
4. **Infrastructure**: database access, external services, messaging, file system.

## Dependency rule
Each layer depends only on the layers below it; the domain depends on nothing. When the application layer needs infrastructure, it depends on an interface defined in the application or domain layer and infrastructure implements it (dependency inversion at that one boundary). Skipping a layer (presentation calling the database) is not allowed.

Configure `architecture.core_path` as the domain (or domain + application) folder and list presentation and infrastructure modules in `architecture.forbidden_core_imports`.

## Where things go
| Piece | Layer |
|---|---|
| Field required, format check | Presentation |
| "Create order and notify customer" flow | Application |
| "An order over the credit limit is rejected" | Domain |
| SQL, ORM mappings, HTTP clients | Infrastructure |

## Common mistakes
- An anemic domain where every rule lives in application services that grow without limit.
- ORM entities used as domain objects and leaked up to the presentation.
- "Utility" layers that every other layer depends on, hiding real dependencies.
- Pass-through layers that add nothing: if a use case is a single call, the application layer may stay very thin, but it still exists.

## Examples by ecosystem
- **Java/Kotlin (Spring)**: `web`, `application`, `domain`, `infrastructure` packages.
- **C# (.NET)**: `Api`, `Application`, `Domain`, `Infrastructure` projects.
- **Python**: `presentation/`, `application/`, `domain/`, `infrastructure/` packages.
- **TypeScript (NestJS)**: controllers, services, domain models and repositories in separate modules.
