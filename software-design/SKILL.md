---
name: software-design
description: The architect role of the development cycle - universal design principles (modularity, separation of responsibilities, SOLID, Clean Code, explicit error handling, design patterns only when needed), the project's architecture style chosen in .dev-cycle.json (hexagonal, layered, MVC, pipeline, modular monolith) with a guide per style, design input for plans, a pre-pull-request review checklist and a checker for forbidden core dependencies. Use when designing or reviewing code structure, choosing or applying an architecture style, planning a change, or before opening a pull request.
---

# 🧭 Software Design Skill (`software-design`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The architect role of the development cycle. It owns **how code is structured**: the principles every change follows, the project's architecture style, the design section of plans, and the structural review before a pull request. The technical documentation of that structure belongs to [`architecture-docs`](../architecture-docs/SKILL.md).

| Phase of the cycle | What this skill does |
|---|---|
| Discovery | Reads `.dev-cycle.json` (`architecture.style`) and the existing structure before proposing anything. |
| Plan | Writes the design section: module boundaries, interfaces, data flow, error strategy, where each new piece lives. |
| Implementation | The principles below apply to every line. |
| Review (before the pull request) | Runs the review checklist and `check_design.py`. |

---

## 🧱 Universal Principles (every project, every style)

### Modularity and separation of responsibilities
- One module, type or function has **one reason to change**. When a description needs "and", split it.
- Group code by what it is about (a domain concept or a feature), not by what kind of file it is, unless the architecture style says otherwise.
- Business rules never live in the user interface, in transport code (HTTP handlers, CLI argument parsing) or in persistence code.
- Dependencies point toward stable code: details (frameworks, databases, network, UI) depend on the core, never the other way around.

### SOLID, in practice
| Principle | What it means when writing code | Smell that it is violated |
|---|---|---|
| **S**ingle responsibility | Each unit does one job and can be named without "and". | A 600-line file mixing parsing, rules and output. |
| **O**pen/closed | Add behavior by adding a new implementation, not by growing an `if/else` chain in stable code. | Every new provider edits the same `match` in five places. |
| **L**iskov substitution | Any implementation of an interface can replace another without surprises. | An implementation that panics or ignores half the contract. |
| **I**nterface segregation | Small interfaces shaped by what the caller needs. | Callers implementing methods they never use. |
| **D**ependency inversion | High-level code depends on abstractions it owns; adapters implement them. | Core code constructing an HTTP client or a database driver. |

### Clean Code
- **Names** say what something is or does; no abbreviations a newcomer cannot decode; no `data`, `info`, `manager`, `util` as whole names.
- **Small functions** that do one thing at one level of abstraction; early returns instead of deep nesting.
- **No magic values**: named constants with units in the name (`TIMEOUT_MS`).
- **Comments explain why**, never what; delete commented-out code and dead code.
- **Consistency over preference**: follow the conventions already in the codebase (naming, error types, layout) unless the change is an agreed refactor.
- **Duplication**: remove it when the same rule exists twice; tolerate it when two pieces only look alike today.

### Explicit error handling
- Never swallow an error (`except: pass`, `let _ =` on a meaningful result, an empty `catch`, a blind `.unwrap()` on input that can fail).
- Errors carry context (what was being done, with which input) and use the project's canonical error types.
- Decide at each boundary: recover, translate into the caller's error type, or propagate. Report to the user once, at the edge.
- Fallbacks are allowed only when they are explicit, logged and visible in status output; a silent fallback that changes behavior is a defect.

### Testability
- Pure logic is separated from I/O so it can be unit-tested without mocks.
- External services sit behind an interface the tests can replace.
- A structure that is hard to test is a design problem, not a testing problem.

---

## ⚖️ Simplicity First (the counterweight)

Principles applied mechanically produce over-engineering. The rule: **the simplest solution that keeps responsibilities separate.**

- Introduce an abstraction (trait, interface, base class, factory) only when at least one of these is true:
  1. there is a **second implementation** now (not "maybe later");
  2. it creates a **test seam** you need today;
  3. it isolates an **external dependency** (framework, service, database, OS API).
- No speculative configuration, plugin systems or generic frameworks for a single use case.
- Prefer composition over inheritance; prefer plain data and functions over class hierarchies.
- Three similar lines are better than a premature helper with five parameters.

### Design patterns: only when they solve a real problem
| Problem actually present | Pattern that fits |
|---|---|
| Several interchangeable algorithms or providers chosen at runtime | Strategy |
| A third-party or legacy interface that does not match yours | Adapter |
| Construction is complex or depends on configuration | Factory / Builder |
| Behavior changes with an explicit lifecycle state | State machine |
| Actions must be queued, logged, undone or dispatched uniformly | Command |
| Persistence must be swappable or mocked | Repository |
| Several parts react to an event without knowing each other | Observer / event channel |

If you cannot point to the problem in the left column, do not use the pattern. Name the pattern in code or docs only when it helps the reader.

---

## 🏛️ Architecture Style (per project)

The project declares its style in `.dev-cycle.json`:

```json
{ "architecture": { "style": "hexagonal", "core_path": "crates/app-core", "surfaces": ["cli", "tui"], "forbidden_core_imports": ["ratatui", "app_cli"] } }
```

| Style | Use when | Guide |
|---|---|---|
| `hexagonal` | A domain core serves several interfaces (CLI, GUI, API, MCP) over one canonical state | [styles/hexagonal.md](styles/hexagonal.md) |
| `layered` | A conventional application with presentation, application, domain and infrastructure layers | [styles/layered.md](styles/layered.md) |
| `mvc` | A web or GUI application organized around a framework's controllers and views | [styles/mvc.md](styles/mvc.md) |
| `pipeline` | Data flows through ordered stages (ETL, compilers, ingestion, batch jobs) | [styles/pipeline.md](styles/pipeline.md) |
| `modular-monolith` | One deployable split into business modules with explicit public interfaces | [styles/modular-monolith.md](styles/modular-monolith.md) |
| `other` | Anything else; describe it in an ADR | — |

**Choosing or changing the style** is an architecture decision: record it with an ADR (`"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs new_adr`) and set `architecture.style`. When the style is not set, infer it from the code during Discovery, propose it to the user and record it once they agree. Read only the guide of the project's style.

Large systems may combine styles (a hexagonal core whose ingestion is a pipeline); the top-level style goes in `.dev-cycle.json` and the combination in the ADR.

In a project that uses Spec Kit, the universal principles, simplicity first and the guide of the project's style are part of the generated constitution (`dev-cycle-protocol/constitution.py`), so Spec Kit's plans are checked against them; after a style change, regenerate it.

---

## 📋 Design Section of a Plan (Phase 2)

Every plan with code changes includes:
1. **Where each new piece lives** (module, layer, file) and why there.
2. **Interfaces** added or changed, with their callers.
3. **Data flow** for the main path, with a Mermaid diagram when more than three components are involved.
4. **Error strategy**: what can fail, where it is handled, what the user sees.
5. **Dependencies**: new libraries or services, and how they are isolated.
6. **What is deliberately not abstracted**, when an abstraction was considered and rejected (simplicity first).

---

## ✅ Review Checklist (before the pull request)

- [ ] Each changed module still has a single responsibility; nothing was added to a file just because it was open.
- [ ] Business rules are in the core/domain, not in UI, transport or persistence code.
- [ ] Dependencies point toward the core; `check_design.py` passes.
- [ ] Every new abstraction meets one of the three conditions in "Simplicity First"; no pattern without its problem.
- [ ] No swallowed errors, no silent fallbacks, no blind unwraps on fallible input.
- [ ] Names, layout and error types follow the existing conventions.
- [ ] No dead code, commented-out code or leftover debug output.
- [ ] New logic is covered by tests (see `quality-assurance`), and pure logic is testable without I/O.
- [ ] Structural or behavioral changes are reflected in `docs/architecture` and, for decisions, in an ADR (`architecture-docs`).

---

## ⚙️ Commands

```powershell
python "<skills_root>/software-design/check_design.py" [--dir <project>] [--strict]
```

Checks, from `.dev-cycle.json`:
- `architecture.style` is set and known (warning when missing);
- `architecture.core_path` exists;
- no source file under `core_path` imports a name listed in `architecture.forbidden_core_imports` (Rust `use`/`extern crate` and fully qualified `crate::` paths, Python `import`/`from`, JavaScript/TypeScript `import`/`require`, Go `import`, C/C++ `#include`, C#/Java `using`/`import`). Test, benchmark and example folders are skipped, since tests may import adapters on purpose.

Exit code 1 on errors; `--strict` also fails on warnings.
