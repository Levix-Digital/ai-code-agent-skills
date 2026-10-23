"""
No AI attribution - dev-cycle-protocol Skill
The guard of protocol principle 9: no artifact names an AI, a model or a coding agent as author,
co-author, reviewer or source. Everything the protocol posts or commits is checked here first.

It looks for attribution, not for a product's subject: "feat: add an OpenAI provider" or
"configure .claude/settings.json" are fine; "Co-Authored-By: Claude", "Generated with ...",
"AI-generated" or "Copilot suggested this" are not. Code spans and file paths are ignored.
"""
import re

AGENTS = r"(?:claude(?:\s+code)?|chat\s*gpt|gpt[\s-]?\d[\w.]*|openai|anthropic|copilot|gemini|codex|cursor|" \
         r"windsurf|devin|llm|large\s+language\s+model|an?\s+ai|the\s+ai|ai(?:\s+(?:assistant|agent|model|tool))?|" \
         r"(?:the|a|an)\s+(?:coding\s+)?agent|the\s+assistant|artificial\s+intelligence)"
PATTERNS = [
    (r"co-authored-by:[^\n]*(?:claude|copilot|chatgpt|gpt|openai|anthropic|gemini|codex|cursor|noreply@anthropic\.com|bot\b)",
     "co-author trailer naming an AI"),
    (r"\bclaude-session:", "session trailer"),
    (r"claude\.ai/code/session", "session link"),
    (r"🤖", "robot attribution emoji"),
    (r"\b(?:generated|written|created|authored|produced|drafted|made|built|reviewed|suggested|assisted|powered)\s+"
     r"(?:with|by|using|via)\s+(?:the\s+help\s+of\s+)?" + AGENTS + r"\b", "attribution to an AI"),
    (r"\bwith\s+(?:the\s+)?help\s+(?:of|from)\s+" + AGENTS + r"\b", "attribution to an AI"),
    (r"\bai[\s-](?:generated|assisted|written|authored|powered\s+review)\b", "attribution to an AI"),
    (r"\bas\s+an?\s+(?:ai|language\s+model|assistant)\b", "speaking as an AI"),
    (r"\b(?:claude|chat\s*gpt|copilot|gemini|codex|cursor|the\s+agent|the\s+assistant)\s+"
     r"(?:found|noticed|suggested|suggests|says|said|thinks|wrote|generated|reviewed|fixed|recommended|recommends)\b",
     "an AI named as the source"),
]
COMPILED = [(re.compile(p, re.I), reason) for p, reason in PATTERNS]


def strip_technical(text: str) -> str:
    """Removes code spans, code blocks, URLs other than session links, and file paths."""
    text = re.sub(r"```.*?```", " ", text, flags=re.S)
    text = re.sub(r"`[^`\n]*`", " ", text)
    text = re.sub(r"(?<![\w/])(?:\.?[\w.-]+/)+[\w.-]+|(?<!\w)\.[\w.-]+\b", " ", text)
    return text


def findings(text: str, allowed: list = None) -> list:
    """[(matched text, reason)] for every attribution found; `allowed` phrases are skipped."""
    plain = strip_technical(text or "")
    out = []
    for regex, reason in COMPILED:
        for m in regex.finditer(text if reason in ("session trailer", "session link") else plain):
            hit = m.group(0).strip()
            if any(a.lower() in hit.lower() for a in allowed or []):
                continue
            out.append((hit, reason))
    return out


def check(text: str, what: str = "text", allowed: list = None):
    """Raises ValueError naming each finding; the text is never posted or committed as it is."""
    found = findings(text, allowed)
    if found:
        listing = "; ".join(f"'{hit}' ({reason})" for hit, reason in found)
        raise ValueError(f"{what} mentions an AI, which protocol principle 9 forbids: {listing}")
