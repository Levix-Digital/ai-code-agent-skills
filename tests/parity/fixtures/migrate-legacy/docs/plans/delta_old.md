---
title: Delta under its old name
---

# Delta plan

Goal: move the cache layer behind a single interface so the storage engine can change later.

## Milestones

1. M1 Interface: define the cache trait and its error type.
2. M2 Adapter: wrap the current store in the new trait.
3. M3 Switch: route every caller through the trait and remove the old calls.

Risks: callers that reach into the store directly; the migration must find them all first.
