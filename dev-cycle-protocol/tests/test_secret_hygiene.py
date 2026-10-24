import os
import subprocess
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from verify_gate import check_secret_hygiene, is_credential_file  # noqa: E402


@pytest.mark.parametrize("path", [
    ".env", ".env.local", ".env.production", "config/prod.env", "id_rsa", "deploy/id_ed25519",
    "credentials.json", ".netrc", "certs/server.pem", "certs/client.p12", "store.pfx",
    "tls/server.key", "android/release.keystore", "client_secret.json", "k8s/secrets.yaml",
    "config/db-secret.yml", "private_key.json", "gcp-private-key.toml", "C:\\repo\\.env",
])
def test_real_credential_files_are_flagged(path):
    assert is_credential_file(path)


@pytest.mark.parametrize("path", [
    # Source code and docs about secrets (the any-context false positives)
    "crates/any-context-core-rs/src/ingestion/secrets.rs",
    "docs/adr/0149-secret-values-are-redacted-before-chunking-with-no-exception.md",
    "docs/plans/any_context_ingestion_secret_files_env_tfvars_key_are_indexed_in_clear.md",
    "src/secret_store.py", "src/private_key_loader.ts", "tests/test_secrets.py",
    # A secret word in a directory, not in the file name
    "secrets/README.md", "private_key/handler.go",
    # Templates
    ".env.example", ".env.sample", "config/.env.template", "secrets.example.yaml",
    "credentials.json.sample",
    # Look-alikes
    "environment.md", "keys.md", "src/env.rs", "monkey.json",
])
def test_code_docs_and_templates_are_not_flagged(path):
    assert not is_credential_file(path)


def git(repo, *args):
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path):
    git(tmp_path, "init", "-q")
    git(tmp_path, "config", "user.email", "test@example.com")
    git(tmp_path, "config", "user.name", "Test")
    return tmp_path


def write(repo, path, text="x\n"):
    full = repo / path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(text, encoding="utf-8")


def test_secret_hygiene_ignores_code_and_docs_about_secrets(repo):
    for path in ["src/ingestion/secrets.rs", "docs/adr/0149-secret-values-are-redacted.md",
                 "docs/plans/secret_files_plan.md", ".env.example"]:
        write(repo, path)
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "init")
    assert check_secret_hygiene(str(repo)) == []


def test_secret_hygiene_reports_committed_and_staged_credentials_once(repo):
    write(repo, "README.md")
    write(repo, "config/client_secret.json")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "init")
    write(repo, ".env")
    write(repo, "certs/server.pem")
    git(repo, "add", "-A")
    assert check_secret_hygiene(str(repo)) == [
        "Tracked credential file committed to Git: config/client_secret.json",
        "Staged credential file ready to be committed: .env",
        "Staged credential file ready to be committed: certs/server.pem",
    ]


def test_secret_hygiene_works_before_the_first_commit(repo):
    write(repo, "id_rsa")
    write(repo, "src/secrets.rs")
    git(repo, "add", "-A")
    assert check_secret_hygiene(str(repo)) == ["Staged credential file ready to be committed: id_rsa"]
