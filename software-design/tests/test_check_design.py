import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from check_design import imported_names, matches_forbidden, run  # noqa: E402


def project(tmp_path, architecture, files):
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"architecture": architecture}), encoding="utf-8")
    for rel, content in files.items():
        path = tmp_path / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    return str(tmp_path)


def test_imported_names_per_language():
    assert imported_names("use ratatui::widgets::Block;") == ["ratatui::widgets::Block"]
    assert imported_names("pub(crate) use crate::store;") == ["crate::store"]
    assert imported_names("extern crate serde;") == ["serde"]
    assert imported_names("from flask import Flask") == ["flask"]
    assert imported_names("import requests") == ["requests"]
    assert "react" in imported_names("import React from 'react';")
    assert imported_names("const x = require('express');") == ["express"]
    assert imported_names('#include <QtWidgets/QWidget>') == ["QtWidgets/QWidget"]
    assert imported_names("using System.Windows.Forms;") == ["System.Windows.Forms"]


def test_matches_forbidden_by_module_prefix_only():
    assert matches_forbidden("ratatui::widgets::Block", "ratatui")
    assert matches_forbidden("actx_cli::tui", "actx-cli")
    assert matches_forbidden("app.web.views", "app.web")
    assert not matches_forbidden("apple::Tree", "app")
    assert not matches_forbidden("crate::store", "ratatui")


def test_clean_core_passes(tmp_path):
    d = project(tmp_path, {"style": "hexagonal", "core_path": "core", "forbidden_core_imports": ["ratatui", "app_cli"]},
                {"core/src/lib.rs": "use crate::domain::Order;\nuse serde::Serialize;\n"})
    errors, warnings = run(d)
    assert errors == [] and warnings == []


def test_forbidden_import_in_core_is_an_error(tmp_path):
    d = project(tmp_path, {"style": "hexagonal", "core_path": "core", "forbidden_core_imports": ["ratatui"]},
                {"core/src/view.rs": "// use ratatui in a comment is fine\nuse ratatui::Frame;\n"})
    errors, _ = run(d)
    assert len(errors) == 1
    assert "core/src/view.rs:2" in errors[0] and "ratatui" in errors[0]


def test_rust_fully_qualified_path_without_use_is_detected(tmp_path):
    d = project(tmp_path, {"style": "hexagonal", "core_path": "core", "forbidden_core_imports": ["app-installer"]},
                {"core/src/engine.rs": "fn bin() { let p = app_installer::paths::bin_dir(); } // app_installer::x\n"
                                       "fn ok() { let my_app_installer = 1; }\n"})
    errors, _ = run(d)
    assert len(errors) == 1 and "engine.rs:1" in errors[0] and "app_installer::paths::bin_dir" in errors[0]


def test_missing_style_and_core_path_are_warnings(tmp_path):
    d = project(tmp_path, {}, {})
    errors, warnings = run(d)
    assert errors == []
    assert any("architecture.style is not set" in w for w in warnings)
    assert any("core_path is not set" in w for w in warnings)


def test_unknown_style_and_missing_core_dir_are_errors(tmp_path):
    d = project(tmp_path, {"style": "spaghetti", "core_path": "nope", "forbidden_core_imports": ["x"]}, {})
    errors, _ = run(d)
    assert any("unknown" in e for e in errors)
    assert any("does not exist" in e for e in errors)


def test_build_folders_are_skipped(tmp_path):
    d = project(tmp_path, {"style": "layered", "core_path": "core", "forbidden_core_imports": ["django"]},
                {"core/node_modules/pkg/index.py": "import django\n", "core/domain.py": "import dataclasses\n"})
    errors, _ = run(d)
    assert errors == []


def test_cli_exit_codes(tmp_path):
    script = os.path.join(SKILL, "check_design.py")
    d = project(tmp_path, {"core_path": "core", "forbidden_core_imports": ["flask"]}, {"core/app.py": "from flask import Flask\n"})
    res = subprocess.run([sys.executable, script, "--dir", d], capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 1 and "forbidden: 'flask'" in res.stdout
    (tmp_path / "core" / "app.py").write_text("import json\n", encoding="utf-8")
    res = subprocess.run([sys.executable, script, "--dir", d], capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 0
    res = subprocess.run([sys.executable, script, "--dir", d, "--strict"], capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 1  # style not set is a warning
