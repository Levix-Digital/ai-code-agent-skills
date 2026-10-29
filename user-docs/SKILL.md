---
name: user-docs
description: The technical writer role of the development cycle - writes, reviews and updates the project's README (and other end-user documentation) for the people who install and use the software, in English only, with a standard structure (what it is, installation, quick start, usage, configuration, troubleshooting, links, license), edited in place without version sections or implementation detail, plus a template and a checker. Use when creating or rewriting a README, when a change affects how users install, run or configure the software, or when reviewing user-facing documentation.
---

# ✍️ User Docs Skill (`user-docs`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The technical writer role of the development cycle. It owns the **README** (`docs.user_doc` in `.dev-cycle.json`, default `README.md`) and any end-user guide. The README answers one question: *how do I install and use this?* How the software works inside belongs to the technical documentation ([`architecture-docs`](../architecture-docs/SKILL.md)); what changed in each version belongs to the release notes.

| Phase of the cycle | What this skill does |
|---|---|
| Plan | States whether the change affects installation, commands, configuration or troubleshooting, and which README sections change. |
| Documentation | Edits those sections in place and runs `check_readme`. |
| Review | Checks that every command and example in the README still works. |

---

## 👥 Audience

Write for a person who has never seen the project: they want to know what it does, whether it is for them, how to install it, how to do the first useful thing, and where to look when something goes wrong. Maintainers are not the audience; link them to the technical documentation.

---

## 🧱 Structure

Use [`README_TEMPLATE.md`](README_TEMPLATE.md). Sections, in this order:

| Section | Content | Required |
|---|---|---|
| `# <Name>` + description | One sentence on what it is and for whom, then a short paragraph with the main benefit. | yes |
| Features | 3 to 8 bullets of what users can do (capabilities, not internals). | recommended |
| Requirements | Supported platforms, runtimes, accounts or keys needed. | recommended |
| Installation | Copyable commands per platform; how to verify the install. | yes |
| Quick Start | The shortest path to a first useful result, in 3 to 6 steps. | yes (or Usage) |
| Usage | The main commands or screens with one example each; link to the full reference if there is one. | yes (or Quick Start) |
| Configuration | Settings, environment variables and files users may change, with defaults. | recommended |
| Updating and Uninstalling | How to update and remove the software. | when it applies |
| Troubleshooting | The most common problems with their fix, and where the logs are. | recommended |
| Documentation | Links to user guides, the technical documentation index and the release notes. | recommended |
| Contributing | How to report issues or contribute, or a link to `CONTRIBUTING.md`. | optional |
| License | License name and link. | yes |

---

## 📏 Rules

1. **English only**, always, whatever the language of the conversation or of other files.
2. **Current state only.** Describe how the software works today. No "What's new", "Changelog", "Recent changes", "Version history" or per-version sections, and no version numbers in headings; the release notes on the releases page cover history. Mention a version only where users need it (a minimum required version, an install command for a specific version).
3. **Edit in place.** A change updates the section it affects; outdated text is replaced or removed, never followed by an "update" paragraph.
4. **No implementation detail.** No internal module, crate, class or function names, no source paths, no architecture diagrams, no ADR references, no algorithm descriptions. Link to the technical documentation index instead.
5. **Copyable and true.** Every command runs as written on the platforms it claims; every example matches the current CLI help or UI. Verify them against the current build before committing.
6. **Short.** Keep the README under `docs.readme_max_lines` (default 400). Long material goes to dedicated guides under `docs/` linked from the Documentation section.
7. **Plain wording.** Short sentences, second person ("you"), active voice, no marketing superlatives, no emoji in headings.
8. **One source per fact.** Configuration defaults, commands and paths are written once; other pages link to that place.

---

## 🔁 Rewriting a Legacy README

For a README that grew into a changelog or a technical manual:

1. **Inventory**: list what the current README claims (commands, flags, paths, features).
2. **Verify against the software**: compare each claim with the current `--help` output, menus and default paths; drop what no longer exists.
3. **Separate**: history goes to the release notes (already on the releases page; nothing to keep in the README); architecture and implementation detail go to the technical documentation (check that `architecture-docs` already covers it).
4. **Rewrite** from the template, section by section, using only verified facts.
5. **Run `check_readme`** until it reports zero errors, and fix or justify each warning.
6. **Review the commands** by running the installation and quick start on a clean environment when possible.

---

## ⚙️ Commands

It runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```sh
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" user-docs check_readme [--dir <project>] [--file README.md] [--max-warnings N]
```
In PowerShell:
```powershell
& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" user-docs check_readme [--dir <project>] [--file README.md] [--max-warnings N]
```

Errors (exit code 1):
- missing title (H1) or description paragraph;
- missing required sections (Installation; Quick Start or Usage; License);
- headings with version numbers or dates, or changelog-style headings ("What's New", "Changelog", "Release Notes", "Recent Changes", "Version History");
- headings not in English;
- broken relative links.

Warnings:
- more lines than `docs.readme_max_lines`;
- missing recommended sections;
- version references in body text (`v1.2.3`) outside commands;
- implementation detail: ADR references, links or paths to source files, architecture diagrams;
- body text that looks like another language.

`--max-warnings N` turns more than N warnings into a failure. Put `<!-- readme-ok -->` on a line to exempt it (for example a required minimum version).
