---
name: speckit
description: Speckit Full SDD Cycle workflow commands (constitution, specify, plan, tasks, implement, converge)
---

# Speckit Workflow

This skill maps the Speckit SDD (Software Design Document) lifecycle directly to Antigravity's native Planning Mode and artifact system. When the user asks to run any of these Speckit commands (e.g., "run speckit.plan", "speckit.tasks", etc.), follow the exact instructions below:

### Core Commands

- **`speckit.constitution`**: Establish project principles. Read the project's current documentation or ask the user for their vision. Write or update these core principles into `.agents\AGENTS.md` at the root of the workspace so they apply globally to the project.
- **`speckit.specify`**: Create a baseline specification. Research the user's request and output a detailed spec document outlining the features, architecture, and requirements. Do not start coding or planning yet.
- **`speckit.plan`**: Enter **Planning Mode**. Create a detailed `implementation_plan.md` artifact outlining the proposed changes, file by file. You MUST set `RequestFeedback: true` in the artifact metadata to pause execution—this acts as the **review gate** to let the user approve the plan.
- **`speckit.tasks`**: Generate actionable tasks. Once the plan is approved, create a `task.md` artifact with a structured checklist of uncompleted tasks (`- [ ]`).
- **`speckit.implement`**: Execute the implementation. Systematically work through the `task.md` items, write the necessary code, run verification commands, and mark tasks as complete (`- [x]`). 
- **`speckit.converge`**: Assess the current state of the codebase against the original spec/plan. Find any remaining work, discrepancies, or bugs, and append them as new tasks in `task.md`.

### Optional Commands (Quality & Confidence)

- **`speckit.clarify`**: Ask structured questions to de-risk ambiguous areas before planning. Use the `ask_question` tool if multiple choices exist, or output a formatted list of clear questions.
- **`speckit.analyze`**: Produce a cross-artifact consistency & alignment report. Ensure the spec, plan, and current codebase remain aligned before implementing.
- **`speckit.checklist`**: Generate a quality checklist to validate requirements completeness, clarity, and consistency (usually run after `speckit.plan`).
