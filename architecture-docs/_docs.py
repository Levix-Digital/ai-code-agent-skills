"""
Shared helpers - architecture-docs Skill
Project layout, configuration, frontmatter, ADR files, GitHub-style anchors and
the language heuristic used by the scripts of this skill.

Layout (configurable in .dev-cycle.json, key "architecture_docs"):
  TECDOC.md                       short index, generated sections between markers
  docs/architecture/<name>.md     living documentation of each subsystem (current state)
  docs/adr/NNNN-<slug>.md         one Architecture Decision Record per file
"""
import datetime
import json
import os
import re
import subprocess
import sys
import unicodedata

import yaml

DEFAULTS = {
    "index": "TECDOC.md",
    "architecture_dir": os.path.join("docs", "architecture"),
    "adr_dir": os.path.join("docs", "adr"),
    "max_lines": 800,
    "language": "en",
    "language_scope": ["index", "architecture", "adr"],
}
ADR_STATUSES = ["proposed", "accepted", "rejected", "deprecated", "superseded"]
ADR_SECTIONS = ["Context", "Decision", "Alternatives Considered", "Consequences"]
ADR_FIELD_ORDER = ["id", "title", "status", "date", "version", "subsystems",
                   "supersedes", "superseded_by", "amends", "amended_by"]
RELATIONS = [("supersedes", "superseded_by"), ("amends", "amended_by")]
MARKER = "<!-- architecture-docs:{name}:{edge} -->"


def setup_stdout():
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass


def today() -> str:
    return datetime.date.today().isoformat()


def ascii_slug(text: str, max_len: int = 60) -> str:
    text = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode("ascii").lower()
    text = re.sub(r"[^a-z0-9]+", "-", text).strip("-")
    return text[:max_len].rstrip("-") or "untitled"


def github_anchor(heading: str) -> str:
    """Anchor GitHub generates for a heading (without the -1, -2 suffix for duplicates)."""
    text = re.sub(r"<[^>]+>", "", heading).strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


# ---------------------------------------------------------------- project

def repo_root(start_dir: str = None) -> str:
    start = os.path.abspath(start_dir or os.getcwd())
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=start,
                             capture_output=True, text=True, check=True).stdout.strip()
        return os.path.abspath(out)
    except Exception:
        return start


class Project:
    def __init__(self, start_dir: str = None):
        self.root = repo_root(start_dir)
        manifest = os.path.join(self.root, ".dev-cycle.json")
        self.manifest = {}
        if os.path.isfile(manifest):
            with open(manifest, "r", encoding="utf-8") as f:
                self.manifest = json.load(f)
        self.config = {**DEFAULTS, **self.manifest.get("architecture_docs", {})}
        self.name = self.manifest.get("project_name") or os.path.basename(self.root)
        self.index_path = os.path.join(self.root, self.config["index"])
        self.architecture_dir = os.path.join(self.root, self.config["architecture_dir"])
        self.adr_dir = os.path.join(self.root, self.config["adr_dir"])
        plans_rel = (self.manifest.get("plans") or {}).get("dir") or self.manifest.get("plans_dir") or os.path.join("docs", "plans")
        self.plans_dir = os.path.join(self.root, plans_rel)

    def rel(self, path: str) -> str:
        return os.path.relpath(path, self.root).replace(os.sep, "/")

    def adopted(self) -> bool:
        """True once the project uses the docs/adr + docs/architecture layout."""
        return os.path.isdir(self.adr_dir) or os.path.isdir(self.architecture_dir)

    def architecture_docs(self) -> list:
        if not os.path.isdir(self.architecture_dir):
            return []
        return [read_doc(os.path.join(self.architecture_dir, n))
                for n in sorted(os.listdir(self.architecture_dir)) if n.endswith(".md")]

    def adrs(self) -> list:
        if not os.path.isdir(self.adr_dir):
            return []
        return [read_doc(os.path.join(self.adr_dir, n))
                for n in sorted(os.listdir(self.adr_dir)) if re.match(r"\d{4}-.*\.md$", n)]

    def adr_by_id(self) -> dict:
        return {d.meta.get("id"): d for d in self.adrs()}

    def next_adr_id(self) -> int:
        ids = [i for i in self.adr_by_id() if isinstance(i, int)]
        return max(ids, default=0) + 1

    def adr_path(self, adr_id: int, title: str) -> str:
        return os.path.join(self.adr_dir, f"{adr_id:04d}-{ascii_slug(title)}.md")


# ---------------------------------------------------------------- documents

class Doc:
    def __init__(self, path: str, meta: dict, body: str, has_frontmatter: bool):
        self.path = path
        self.meta = meta
        self.body = body
        self.has_frontmatter = has_frontmatter

    @property
    def h1(self):
        m = re.search(r"^# (.+)$", self.body, re.M)
        return m.group(1).strip() if m else None


def _plain(value):
    if isinstance(value, (datetime.datetime, datetime.date)):
        return value.isoformat()
    if isinstance(value, dict):
        return {k: _plain(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_plain(v) for v in value]
    return value


def split_frontmatter(text: str):
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end != -1:
            return _plain(yaml.safe_load(text[4:end]) or {}), text[end + 5:].lstrip("\n"), True
    return {}, text, False


def read_doc(path: str) -> Doc:
    with open(path, "r", encoding="utf-8") as f:
        meta, body, has = split_frontmatter(f.read())
    return Doc(path, meta, body, has)


def write_doc(doc: Doc, order: list = None) -> None:
    meta = doc.meta
    if order:
        meta = {k: doc.meta[k] for k in order if k in doc.meta}
        meta.update({k: v for k, v in doc.meta.items() if k not in meta})
    front = yaml.safe_dump(meta, sort_keys=False, allow_unicode=True, default_flow_style=None, width=4096)
    os.makedirs(os.path.dirname(doc.path), exist_ok=True)
    with open(doc.path, "w", encoding="utf-8", newline="\n") as f:
        f.write(f"---\n{front}---\n\n{doc.body.strip()}\n")


def as_id_list(value) -> list:
    if value in (None, "", []):
        return []
    if not isinstance(value, list):
        value = [value]
    out = []
    for v in value:
        m = re.search(r"\d+", str(v))
        if m:
            out.append(int(m.group()))
    return out


def adr_label(adr_id: int) -> str:
    return f"ADR-{adr_id:04d}"


# ---------------------------------------------------------------- markdown

FENCE_RE = re.compile(r"^\s{0,3}(`{3,}|~{3,})(.*)$")


def fenced_lines(lines: list) -> list:
    """For each line, True when it is part of a fenced code block (CommonMark rules:
    a block closes only with the same fence character, at least as long, and nothing after it)."""
    flags, fence = [], None
    for line in lines:
        m = FENCE_RE.match(line)
        if fence is None:
            if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
                fence = m.group(1)
                flags.append(True)
            else:
                flags.append(False)
        else:
            flags.append(True)
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence) and not m.group(2).strip():
                fence = None
    return flags


def strip_code(text: str) -> str:
    """Removes fenced code blocks, inline code and URLs (kept as blank lines to preserve numbering)."""
    lines = text.split("\n")
    out = []
    for line, fenced in zip(lines, fenced_lines(lines)):
        if fenced:
            out.append("")
            continue
        line = re.sub(r"`[^`]*`", " ", line)
        line = re.sub(r"\]\([^)]*\)", "]", line)
        line = re.sub(r"https?://\S+", " ", line)
        out.append(line)
    return "\n".join(out)


def strip_code_keep_headings(text: str) -> str:
    lines = text.split("\n")
    return "\n".join("" if fenced else line for line, fenced in zip(lines, fenced_lines(lines)))


def headings(text: str) -> list:
    """(line_number, level, title) for every Markdown heading outside code blocks."""
    result = []
    for n, line in enumerate(strip_code_keep_headings(text).split("\n"), 1):
        m = re.match(r"^(#{1,6})\s+(.+?)\s*#*\s*$", line)
        if m:
            result.append((n, len(m.group(1)), m.group(2)))
    return result


def anchors(text: str) -> set:
    seen, result = {}, set()
    for _, _, title in headings(text):
        base = github_anchor(title)
        count = seen.get(base, 0)
        result.add(base if count == 0 else f"{base}-{count}")
        seen[base] = count + 1
    return result


# ---------------------------------------------------------------- language

PT_ACCENTED = re.compile(r"\b\w*[ãõçâêôáéíóúàÃÕÇÂÊÔÁÉÍÓÚÀ]\w*\b")
PT_WORDS = {
    "não", "nao", "para", "com", "uma", "umas", "dos", "das", "que", "pelo", "pela", "pelos", "pelas",
    "são", "sao", "também", "tambem", "está", "esta", "estão", "quando", "então", "entao", "sobre",
    "entre", "após", "apos", "pois", "como", "mais", "muito", "isso", "este", "esse", "essa", "nosso",
    "nossa", "deve", "devem", "foi", "foram", "será", "sera", "seja", "sem", "porque", "onde", "ainda",
    "cada", "todo", "toda", "todos", "todas", "mesmo", "agora", "antes", "depois", "desta", "deste",
    "nesta", "neste", "numa", "num", "aos", "às", "ao", "nos", "nas", "usuário", "usuario", "arquivo",
    "arquivos", "pasta", "versão", "versao", "decisão", "decisao", "contexto", "consequências",
    "correção", "correcao", "implementação", "implementacao", "atualização", "atualizacao",
}
ENGLISH_OK = {"com", "as", "a", "e", "o", "nos"}  # too ambiguous on their own


def foreign_language_hits(line: str, language: str = "en") -> list:
    """Words suggesting the line is not in English. Only Portuguese is detected for now."""
    if language != "en":
        return []
    words = re.findall(r"[^\W\d_]+", line.lower())
    accented = [w for w in PT_ACCENTED.findall(line)]
    marked = [w for w in words if w in PT_WORDS and w not in ENGLISH_OK]
    if accented:
        return sorted(set(accented))[:5]
    if len(set(marked)) >= 2:
        return sorted(set(marked))[:5]
    return []
