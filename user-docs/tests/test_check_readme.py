import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from check_readme import check_text, run  # noqa: E402

GOOD = """# Demo Tool

Demo Tool turns notes into searchable answers for small teams.

It indexes your folders locally and answers questions from them.

## Features

- Ask questions about your documents

## Requirements

- Windows 10 or later

## Installation

```bash
curl -fsSL https://example.com/install.sh | sh
demo --version
```

## Quick Start

1. Run `demo add ./notes`.
2. Ask `demo "what changed last week?"`.

## Usage

```bash
demo sync
```

## Configuration

| Setting | Default |
|---|---|
| `DEMO_HOME` | `~/.demo` |

## Troubleshooting

Logs are in `~/.demo/logs`.

## Documentation

- [Technical documentation](docs/TECH.md)

## License

MIT - see [LICENSE](LICENSE).
"""


def check(text, tmp_path, max_lines=400):
    path = tmp_path / "README.md"
    path.write_text(text, encoding="utf-8")
    (tmp_path / "LICENSE").write_text("MIT", encoding="utf-8")
    (tmp_path / "docs").mkdir(exist_ok=True)
    (tmp_path / "docs" / "TECH.md").write_text("# Tech", encoding="utf-8")
    return check_text(str(path), text, max_lines)


def test_good_readme_has_no_findings(tmp_path):
    res = check(GOOD, tmp_path)
    assert res.errors == [] and res.warnings == []


def test_changelog_and_version_headings_are_errors(tmp_path):
    text = GOOD + "\n## What's New\n\n- Faster sync\n\n### Hybrid search (`v0.32.4`)\n\nDetails.\n\n## Changelog\n"
    res = check(text, tmp_path)
    msgs = "\n".join(res.errors)
    assert "changelog-style heading 'What's New'" in msgs
    assert "version or date in heading" in msgs
    assert "changelog-style heading 'Changelog'" in msgs


def test_legitimate_usage_headings_are_not_changelog(tmp_path):
    text = GOOD.replace("## Usage", "## Usage\n\n### Prompt history\n\nUse the arrow keys.\n\n### Updating\n\nRun `demo update`.\n")
    assert check(text, tmp_path).errors == []


def test_missing_required_sections_and_description(tmp_path):
    res = check("# Demo\n\n## Notes\n\nSomething.\n", tmp_path)
    msgs = "\n".join(res.errors)
    assert "missing required section: Installation" in msgs
    assert "missing required section: Quick Start or Usage" in msgs
    assert "missing required section: License" in msgs
    res = check("# Demo\n## Installation\n", tmp_path)
    assert any("missing description" in e for e in res.errors)


def test_portuguese_heading_is_error_and_text_is_warning(tmp_path):
    text = GOOD.replace("## Troubleshooting", "## Solução de Problemas") + "\nEste parágrafo não está em inglês.\n"
    res = check(text, tmp_path)
    assert any("heading not in English" in e for e in res.errors)
    assert any("text may not be in English" in w for w in res.warnings)


def test_implementation_detail_and_versions_in_body_are_warnings(tmp_path):
    text = GOOD + "\nSee ADR-0130 and [the engine](crates/core/src/engine.rs). Added in v0.45.0.\n\n```mermaid\nflowchart LR\n A-->B\n```\n"
    (tmp_path / "crates" / "core" / "src").mkdir(parents=True)
    (tmp_path / "crates" / "core" / "src" / "engine.rs").write_text("", encoding="utf-8")
    msgs = "\n".join(check(text, tmp_path).warnings)
    assert "ADR reference" in msgs
    assert "link to a source file" in msgs
    assert "version reference in body text" in msgs
    assert "diagram in the README" in msgs


def test_exempt_marker_and_versions_in_code(tmp_path):
    text = GOOD + "\nRequires Python 3.10.0 or later. <!-- readme-ok -->\n\n```bash\npip install demo==1.2.3\n```\n"
    assert check(text, tmp_path).warnings == []


def test_broken_links_and_anchors(tmp_path):
    text = GOOD + "\nSee [missing](docs/NOPE.md) and [here](#no-such-section) and [ok](#installation).\n"
    msgs = "\n".join(check(text, tmp_path).errors)
    assert "broken link: docs/NOPE.md" in msgs
    assert "broken anchor: #no-such-section" in msgs
    assert "#installation" not in msgs


def test_length_limit_from_config(tmp_path):
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"docs": {"readme_max_lines": 20}}), encoding="utf-8")
    check(GOOD, tmp_path)  # writes README.md, LICENSE and docs/TECH.md
    res = run(str(tmp_path))
    assert any("limit 20" in w for w in res.warnings)


def test_cli_max_warnings(tmp_path):
    check(GOOD.replace("## Features\n\n- Ask questions about your documents\n\n", ""), tmp_path)
    script = os.path.join(SKILL, "check_readme.py")
    ok = subprocess.run([sys.executable, script, "--dir", str(tmp_path)], capture_output=True, text=True, encoding="utf-8")
    assert ok.returncode == 0, ok.stdout
    strict = subprocess.run([sys.executable, script, "--dir", str(tmp_path), "--max-warnings", "0"],
                            capture_output=True, text=True, encoding="utf-8")
    assert strict.returncode == 1 and "missing recommended section: Features" in strict.stdout
