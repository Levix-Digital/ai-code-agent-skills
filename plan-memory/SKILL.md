---
name: plan-memory
description: Persistent vector storage and semantic retrieval of complete, uncompressed project architecture plans, roadmaps, and multi-step blueprints in ChromaDB. Ensures zero loss of engineering plans, with rich metadata (project, session_id, version, status, milestones) and lifecycle tracking. Project-agnostic.
---

# 📐 Project Plan Memory Skill (`plan-memory`)

> **Caminhos**: `<skills_root>` é a pasta que contém esta skill (o diretório pai da pasta deste `SKILL.md`). Use o caminho absoluto real ao executar os comandos.

The **`plan-memory`** skill provides persistent, vector-indexed storage, lifecycle management, and semantic retrieval of **100% complete, uncompressed architectural plans, strategic roadmaps, and multi-step blueprints** across any software project.

- **ChromaDB Path**: `<skills_root>/plan-memory/db`
- **Collection Name**: `project_plans`
- **Guiding Principle**: **Zero Compaction / Zero Loss**. Architectural decisions and blueprints must be preserved in their entirety (including Mermaid diagrams, tables, steps, formulas, and rationale) so they serve as persistent knowledge bases across active and future coding sessions.

---

## 🏛️ Architecture & ChromaDB Schema

Every plan entry in the `project_plans` collection preserves the full uncompressed Markdown document alongside rich structured metadata:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        PROJECT PLAN MEMORY SCHEMA                                      │
├────────────────────┬───────────────────────────────────────────────────────────────────┤
│ Field              │ Description & Purpose                                             │
├────────────────────┼───────────────────────────────────────────────────────────────────┤
│ `id`               │ Unique Plan Identifier (e.g. `anycontext_master_roadmap_v0_30_37`) │
│ `document`         │ 100% Full Uncompressed Markdown content of the plan               │
│ `project`          │ Project Name (e.g. "AnyContext", "Intellexis", "Copyx")           │
│ `title`            │ Human-readable headline of the plan                               │
│ `session_id`       │ Conversation ID where the plan was conceived / approved           │
│ `version`          │ Target software version (e.g. "v0.30.37")                         │
│ `status`           │ Lifecycle status: `active`, `in_progress`, `completed`, `superseded`│
│ `created_at`       │ ISO 8601 creation timestamp                                       │
│ `updated_at`       │ ISO 8601 last update timestamp                                    │
│ `tags`             │ Comma-separated search keywords                                   │
│ `milestones`       │ Comma-separated target milestones                                 │
│ `completed_ms`     │ Comma-separated list of checked-off milestones                    │
│ `supersedes`       │ Older Plan ID that this plan replaces / deprecates                │
│ `superseded_by`    │ Newer Plan ID that superseded this plan                           │
│ `parent_plan_id`   │ Optional parent Plan ID for hierarchical planning trees           │
│ `status_notes`     │ Timestamped progress log and architectural notes                  │
│ `content_length`   │ Character count of the uncompressed plan                          │
└────────────────────┴───────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Operational Commands & Scripts

The skill provides a complete suite of Python utilities located in `<skills_root>/plan-memory`:

### 1. Salvar / Atualizar um Plano com Linhagem (`save_plan.py`)
Salva o plano integral sem cortes ou resumos. Suporta `--supersedes` para automaticamente arquivar um plano anterior como `superseded`:
```powershell
python "<skills_root>/plan-memory/save_plan.py" `
  --project "AnyContext" `
  --title "Roadmap Integrado: Python -> Rust, ModelRouter & Kit de Models" `
  --file "caminho\para\plano.md" `
  --session-id "9f488dbf-db54-4ecb-8ed2-b7a6ac1b2372" `
  --version "v0.30.37" `
  --tags "roadmap,rust_refactor,model_router,kit_de_models" `
  --milestones "Prioridade 2B,Prioridade 3,Prioridade 4,Marco 2,Marco 3,Kit de Models" `
  --status "active" `
  --supersedes "anycontext_old_roadmap_v0_30_0"
```

### 2. Rastreador Granular de Marcos & Progresso (`check_milestone.py`)
Controla o avanço das sub-etapas e renderiza barras de progresso visuais:
```powershell
# Ver o progresso e checklist de marcos do plano ativo:
python "<skills_root>/plan-memory/check_milestone.py" --project "AnyContext" --list

# Marcar um marco como concluído:
python "<skills_root>/plan-memory/check_milestone.py" `
  --project "AnyContext" `
  --milestone "Prioridade 2B" `
  --notes "Concluído via PR #319 com suíte de testes 100% verde"

# Desmarcar um marco (reverter para pendente se necessário):
python "<skills_root>/plan-memory/check_milestone.py" `
  --project "AnyContext" `
  --milestone "Prioridade 2B" `
  --uncheck
```

### 3. Detector de Desvios Arquiteturais (`detect_drift.py`)
Compara qualquer proposta de implementação, issue ou novo prompt contra o plano ativo e contra princípios anti-pattern no ChromaDB:
```powershell
# Verificar alinhamento arquitetural de uma proposta:
python "<skills_root>/plan-memory/detect_drift.py" `
  --project "AnyContext" `
  --proposal "Implementacao da Prioridade 2B: construir o orquestrador ReAct nativo em Rust integrando com actx-lm, substituindo o LangGraph do Python com zero frameworks pesados"

# Verificar proposta a partir de arquivo com modo estrito (exit code != 0 em conflito):
python "<skills_root>/plan-memory/detect_drift.py" `
  --project "AnyContext" `
  --file "caminho\para\proposta.md" `
  --strict
```

### 4. Exportador Automatizado de Snapshots (`export_plan.py`)
Exporta planos do ChromaDB para arquivos Markdown com cabeçalho YAML frontmatter padronizado ou arquivos JSON para o repositório:
```powershell
# Exportar Markdown com frontmatter para docs/plans/:
python "<skills_root>/plan-memory/export_plan.py" `
  --project "AnyContext" `
  --output-dir "C:\Users\guilh\source\repos\any-context\docs\plans"

# Exportar snapshot em JSON:
python "<skills_root>/plan-memory/export_plan.py" `
  --id "anycontext_master_roadmap_v0_30_37" `
  --output "docs\plans\roadmap.json" `
  --format json
```

### 5. Listar Planos com Barra de Progresso e Linhagem (`list_plans.py`)
Permite visualizar todos os planos ou filtrar por projeto e status:
```powershell
# Listar todos os planos
python "<skills_root>/plan-memory/list_plans.py"

# Filtrar por projeto e status
python "<skills_root>/plan-memory/list_plans.py" --project "AnyContext" --status "active"
```

### 6. Recuperar Plano Completo na Íntegra (`get_plan.py`)
Recupera o texto completo sem truncamento para orientar a sessão ou exportar para documentação:
```powershell
# Imprimir no terminal com cabeçalho de marcos e status
python "<skills_root>/plan-memory/get_plan.py" --id "<plan_id>"

# Obter o último plano ativo do projeto
python "<skills_root>/plan-memory/get_plan.py" --project "AnyContext"
```

### 7. Busca Semântica em Planos (`search_plans.py`)
Pesquisa planos por proximidade vetorial baseada no conceito ou palavra-chave:
```powershell
python "<skills_root>/plan-memory/search_plans.py" --query "como implementar model router no chat" --project "AnyContext"
```

### 8. Atualizar Status do Ciclo de Vida (`update_plan_status.py`)
Quando etapas forem concluídas ou novos planos substituírem anteriores:
```powershell
python "<skills_root>/plan-memory/update_plan_status.py" `
  --id "<plan_id>" `
  --status "completed" `
  --notes "Prioridade 2B e Prioridade 3 entregues com sucesso."
```

---

## 🔒 Integração Obrigatória com o `dev-cycle-protocol`

Todo o ciclo de vida de desenvolvimento segue a governança do:
👉 **[`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md)**

- **Gatilho de Gravação (Etapa 2: Planejamento Final Completo)**:
  Sempre que um **Planejamento Final Completo (Blueprint)** ou **Roadmap Estratégico** for formulado na Etapa 2, o assistente **DEVE** indexar o plano integralmente via `save_plan.py` no ChromaDB antes de submeter ao Gate de Aprovação da Etapa 3. Caso substitua um plano anterior, use `--supersedes`.
- **Verificação de Desvio (Etapa 2 / Pré-Gate)**:
  Use `detect_drift.py` para assegurar que a proposta não viola diretrizes invioláveis nem inverte a ordem de execução dos marcos.
- **Consulta Prévia de Planos (Etapa 1: Discovery & Exploração)**:
  Ao iniciar a discussão de novas funcionalidades ou prioridades, o assistente **DEVE** consultar `list_plans.py`, `check_milestone.py --list` ou `search_plans.py` para verificar o progresso real dos marcos e diretrizes já acordadas.
- **Conclusão de Marcos (Etapa 8: Release)**:
  Ao fechar a release, use `check_milestone.py` para marcar a etapa como entregue ou `update_plan_status.py` para finalizar o plano.
