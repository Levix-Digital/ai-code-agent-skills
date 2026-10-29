# Project Configuration: `.dev-cycle.json`

Optional file at a project's root: the project configuration shared by every skill. Each skill owns one section, and every key is optional. `project_config` (`crates/kernel/src/project_config.rs`, and `dev-cycle-protocol/project_config.py` for the Python scripts until they are translated) is the single loader: it fills defaults, accepts the legacy flat keys of older files, and validates the result.

```json
{
  "project_name": "my-app",
  "architecture": {
    "style": "hexagonal",
    "core_path": "src/core",
    "surfaces": ["cli", "rest-api", "mcp"],
    "canonical_datastore": "data/app.db"
  },
  "test_commands": ["cargo test --workspace"],
  "docs": { "user_doc": "README.md", "tec_doc": "TECDOC.md" },
  "tests": { "manual_tests": "tests/MANUAL_TESTS.md" },
  "devops": {
    "branching": "integration",
    "integration_branch": "dev",
    "production_branch": "main",
    "releases_repo": "my-org/my-app-releases",
    "release_candidates": true,
    "release_branch_prefix": "release",
    "version_files": ["Cargo.toml"],
    "observability": "cli"
  },
  "plans": { "dir": "docs/plans" },
  "backlog": { "file": "docs/BACKLOG.md", "blocking_severities": ["critical", "high", "medium"], "bug_deadline": "next_release" },
  "roadmap": { "file": "docs/ROADMAP.md" },
  "product": { "vision": "docs/product/VISION.md", "tech_spec": "docs/product/TECH_SPEC.md", "max_lines": 400 },
  "review": { "provider": "auto", "host": null, "marker": "[review]", "reply_marker": "[reply]", "auto_review": true, "allowed_terms": [] },
  "speckit": {
    "enabled": true,
    "version": "v1.2.3",
    "integration": "claude",
    "script": "sh",
    "branch_prefix": "feat"
  },
  "gedo": { "enabled": false, "feedback": true, "feedback_dir": "docs/gedo-feedback", "repo": "Levix-Digital/gedo" },
  "architecture_docs": {
    "index": "TECDOC.md",
    "architecture_dir": "docs/architecture",
    "adr_dir": "docs/adr",
    "max_lines": 800,
    "language": "en",
    "language_scope": ["index", "architecture", "adr"]
  },
  "drift_rules": [
    { "tech": "langchain", "conflict_msg": "The baseline plan removes LangChain in favor of a native implementation." },
    {
      "tech": "parallel database",
      "negative_markers": ["single source", "settings.db"],
      "proposal_allow": [],
      "conflict_msg": "Proposal hints at a parallel database, violating the single source of truth."
    }
  ]
}
```

| Key | Meaning | Default |
|---|---|---|
| `project_name` | Project name (normalized to kebab-case by plan-memory) | repository folder name |
| `architecture.style` | `hexagonal`, `layered`, `mvc`, `pipeline`, `modular-monolith` or `other` | not set |
| `architecture.core_path` | Where the domain core lives | not set |
| `architecture.surfaces` | Interfaces that must stay in parity | `[]` |
| `architecture.canonical_datastore` | The single source of truth database | not set |
| `architecture.forbidden_core_imports` | Libraries or modules the core must never import (checked by `software-design/check_design.py`) | `[]` |
| `test_commands` | Commands that run the automated tests | auto-detected |
| `docs.user_doc`, `docs.tec_doc` | User documentation and technical documentation index | `README.md`, `TECDOC.md` |
| `docs.readme_max_lines` | Line limit checked by `user-docs check_readme` | `400` |
| `tests.manual_tests` | Curated manual test suite | `tests/MANUAL_TESTS.md` |
| `devops.branching` | Branching model: `integration` (`main` + `dev`, the user merges `dev` into `main`), `trunk` (`main` alone, the user promotes the accepted candidate) or `release-branches` (`main` + `release/X.Y`, the user promotes the accepted candidate). Written by `/dev-cycle` with the user's answer; see `devops-release` | `integration` |
| `devops.integration_branch`, `devops.production_branch` | Branch that receives feature work (`integration` model only), branch that holds released code | `dev`, `main` |
| `devops.release_branch_prefix` | Prefix of the release branches of the `release-branches` model (`<prefix>/X.Y`) | `release` |
| `devops.releases_repo` | Repository where releases are published | not set |
| `devops.release_candidates` | Publish `vX.Y.Z-rc.N` pre-releases for manual acceptance before the final release | `true` |
| `devops.version_files` | Files that hold the version number | `[]` |
| `devops.observability` | `cli`, `service`, `library`, `desktop` or `none` | not set |
| `plans.dir` | Folder of the plan files | `docs/plans` |
| `backlog.file` | The project backlog | `docs/BACKLOG.md` |
| `backlog.blocking_severities` | Severities of open bugs that block plan approval of other work (bugs first); `[]` turns the rule off | `["critical", "high", "medium"]` |
| `backlog.bug_deadline` | Deadline of the open bugs whose severity does not block: `next_release` (they must be in the first release of the roadmap not released, or gates 3 and 8 report them as not triaged) or `off` | `"next_release"` |
| `roadmap.file` | The roadmap of releases (`plan-memory/roadmap.py`) | `docs/ROADMAP.md` |
| `product.vision`, `product.tech_spec` | The business document and the technical specification of Phase 0 (`product-inception`) | `docs/product/VISION.md`, `docs/product/TECH_SPEC.md` |
| `product.max_lines` | Line limit of each product document | `400` |
| `review.provider`, `review.host` | Platform of the pull requests: `auto` (from the `origin` remote), `github`, `gitlab` or `none`; the host for a self-hosted instance | `auto`, from the remote |
| `review.marker`, `review.reply_marker` | Prefixes of the protocol's review findings and answers on the platform | `[review]`, `[reply]` |
| `review.auto_review` | Review every pull request when it opens and after pushes that change code (`code-review`) | `true` |
| `review.allowed_terms` | Phrases the principle 9 guard accepts because they are the project's subject | `[]` |
| `speckit.enabled` | Use GitHub Spec Kit for the detail of feature plans (Phases 1, 2 and 4). Written by `/dev-cycle` with the user's answer; an explicit `false` is a decision and is not asked again, while a missing key means it was never asked | `false` |
| `speckit.version` | Exact Spec Kit release tag the project is pinned to; set by `speckit_setup.py init` and `upgrade` | not set |
| `speckit.integration` | Coding agent integration passed to `specify init` | `claude` |
| `speckit.script` | Script type of Spec Kit's commands: `sh`, `ps` or `py` | `sh` (`ps` on Windows) |
| `speckit.branch_prefix` | Prefix of Spec Kit's feature branches (`<prefix>/NNN-slug`) | `feat` |
| `gedo.enabled` | The project uses [Gedo](https://github.com/Levix-Digital/gedo); the user's decision, never turned on by the agent | `false` |
| `gedo.feedback` | While Gedo is used, record what goes wrong with it (`gedo-feedback`); temporary, until Gedo is mature | `true` |
| `gedo.feedback_dir` | Folder of the Gedo feedback entries, one file each | `docs/gedo-feedback` |
| `gedo.repo` | Repository that receives the Gedo issues | `Levix-Digital/gedo` |
| `architecture_docs` | Layout, line limit and language of the technical docs; add `"plans"` to `language_scope` to check plans too | see `architecture-docs` |
| `drift_rules` | Technologies or patterns the project's plans reject | `[]` |

Older files with flat keys keep working: `core_path`, `surfaces`, `canonical_datastore`, `releases_repo`, `plans_dir` and `doc_files.{user_doc,tec_doc,manual_tests}` are read into their sections, and the sectioned key wins when both exist.

```bash
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol project_config              # effective configuration
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol project_config --validate   # exit code 1 on problems
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol project_config --migrate --write   # move flat keys into sections
```

In PowerShell, run `& ~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle.ps1` instead of `bin/dev-cycle`.

## How a Drift Rule Works

A conflict is raised when:
1. the active plan mentions `tech` together with a rejection marker; and
2. the proposal mentions `tech` without an allowed phrase.

When `negative_markers` and `proposal_allow` are omitted, defaults are used (`remove`, `replace`, `without`, `forbidden`, ... and `without <tech>`, `remove <tech>`, `replace <tech>`, ...). The defaults also include the Portuguese equivalents, needed while existing plans are not yet translated.
