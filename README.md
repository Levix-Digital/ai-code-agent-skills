# AI Code Agent Skills

Skills no formato aberto `SKILL.md` (uma pasta por skill, com frontmatter `name` e `description`), compartilhadas entre agentes de código: Claude Code, Codex, Gemini CLI, Copilot e outros.

## Skills

| Skill | Escopo | No plugin do Claude |
|---|---|---|
| `dev-cycle-protocol` | Ciclo de desenvolvimento em 8 etapas com gates de aprovação | ✅ |
| `plan-memory` | Planos de arquitetura persistidos no ChromaDB, por projeto | ✅ |
| `session-memory` | Memória de sessão em 2 camadas no ChromaDB, por projeto | ✅ |
| `folder-sync` | RAG incremental de pastas com ChromaDB | ✅ |
| `specify`, `speckit*` | Fluxo Spec Kit (SDD) | ✅ |
| `safe-shutdown` | Desligamento seguro do Windows (só local) | ❌ |
| `assistente-diario` | Assistente pessoal (só local, **fora do Git**) | ❌ |

Os dados (bases ChromaDB, `.venv`, `node_modules`, `.env`, sessões e tokens) nunca são versionados: veja o [`.gitignore`](.gitignore).

## Instalação local (todos os agentes)

Cria um link de cada skill na pasta de skills de cada agente. Por padrão: `~/.claude/skills` (Claude Code) e `~/.agents/skills` (local padrão entre agentes).

```powershell
.\install.ps1
.\install.ps1 -Targets "$HOME\.claude\skills","$HOME\.agents\skills","$HOME\.copilot\skills"
```

```bash
./install.sh
./install.sh ~/.claude/skills ~/.agents/skills ~/.gemini/skills
```

Os scripts nunca sobrescrevem uma pasta existente que não seja um link para este repositório. Rode de novo depois de criar uma skill nova.

## Claude Code na nuvem (plugin)

Este repositório também é um marketplace de plugins do Claude Code (`levix-agent-skills`) com o plugin `dev-skills`. Para uma sessão na nuvem carregar as skills, declare no `.claude/settings.json` do projeto:

```json
{
  "extraKnownMarketplaces": {
    "levix-agent-skills": {
      "source": { "source": "github", "repo": "<owner>/<repo>" }
    }
  },
  "enabledPlugins": {
    "dev-skills@levix-agent-skills": true
  }
}
```

No plugin, as skills aparecem com prefixo: `dev-skills:session-memory`.

## Dados das skills de memória

- `session-memory`: `~/.ai-code-agent-memory/session-memory/db` (variável `SESSION_MEMORY_DB`)
- `plan-memory`: ainda em `plan-memory/db`, dentro da pasta da skill (ignorada pelo Git)
