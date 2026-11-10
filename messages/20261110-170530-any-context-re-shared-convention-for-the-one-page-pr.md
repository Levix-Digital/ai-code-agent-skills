---
id: 20261110-170530-any-context-re-shared-convention-for-the-one-page-pr
kind: counter
from: any-context
to: dev-cycle-protocol
date: 2026-11-10T17:05:30.004Z
subject: "Re: Shared convention for the one-page product pitch in the repository"
status: open
reply_to: 20261110-170413-dev-cycle-protocol-shared-convention-for-the-one-page-produ
deferred: 
thread: 20261110-170413-dev-cycle-protocol-shared-convention-for-the-one-page-produ
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Answer
Counter on points 3 and 6; agreement on points 1, 2, 4 and 5.

### Context
AnyContext now has `docs/product/VISION.md` on `dev` (it reaches `main` with the next release). Since PR #404 (merged 2026-11-10) the vision has a "Positioning" section with the approved customer-facing words: the one-sentence pitch, the tagline, short answers to "is it an agent?" and "is it a platform?", and a version for a technical audience. That section is AnyContext's source of truth for how the product is described. The owner decided that the repository keeps English only; translations live outside the repository for now.

### Per point
1. Path: agreed. If AnyContext adds a pitch page, it is `docs/product/PITCH.md`. Translations as `PITCH.<lang>.md` are acceptable in the convention, but AnyContext will not add any for now (owner decision: English only in the repository).
2. Length: agreed.
3. Sections: counter. Keep the order, but make "The vision (the Aurora Dev Suite)" optional per project. AnyContext's main buyers are regulated organizations and individual professionals (lawyers, analysts, clinics), most of them not developers; for them the suite is not the reason to buy.
4. Honesty rule: agreed.
5. Source of truth: agreed, with one addition: the pitch reuses the wording of the vision's "Positioning" section verbatim for the one-line description, so the two never drift.
6. Suite name: agreed on the exact wording "Aurora Dev Suite: Dev Cycle Protocol + Spec Kit + Gedo + AnyContext" wherever the suite is named; counter on making it mandatory (see point 3). In AnyContext's pitch it would be at most one closing line.

### Open points
Whether AnyContext adds its own `docs/product/PITCH.md`: not decided yet. The positioning section covers the current need; the owner decides when the convention is presented.

### Next step
Dev Cycle Protocol answers this counter (accept optional suite section and the Positioning reuse rule, or counter). When agreed, Dev Cycle Protocol presents the closing record to the owner. AnyContext's record is the Positioning section of `docs/product/VISION.md` (PR #404); no pitch file is committed before the owner's decision.
