---
name: session-memory
description: Persistent multi-project vector memory and context recovery for coding-agent sessions. Keeps, per project, a 2-layer rolling memory in a shared local ChromaDB (20-turn incremental checkpoints + one consolidated Master Summary), every record tagged with its project name. Use at the start of a session to recover a project's context, every 20 turns to checkpoint, and at the end of a session to consolidate.
---

# 🧠 Session Memory Skill (`session-memory`)

Banco vetorial **ChromaDB local e compartilhado** que guarda, **por projeto**, contexto, decisões de arquitetura, correções, histórico de releases e próximos passos de sessões de desenvolvimento. Funciona com qualquer agente de código e qualquer projeto.

- **ChromaDB**: `~/.ai-code-agent-memory/session-memory/db` (sobrescreva com a variável `SESSION_MEMORY_DB`). Fica fora da pasta da skill, então a skill pode ser versionada e copiada sem levar dados.
- **Collection**: `session_history`
- **Scripts**: na mesma pasta deste `SKILL.md` (abaixo, `<skill_dir>`). Requer `pip install chromadb`.

> [!CAUTION]
> **REGRA DE SEGURANÇA CRÍTICA DO SISTEMA**:
> **JAMAIS reiniciar ou desligar o computador sem um pedido EXPLÍCITO e DIRETO do usuário na sessão ativa.**
> É terminantemente proibido qualquer reboot ou shutdown autônomo, proativo ou como consequência de testes, builds, deploys ou atualizações.

---

## 🏷️ Isolamento por Projeto (obrigatório)

Todo registro carrega o metadado `project` e um ID com namespace do projeto:

| Registro | Doc ID | Metadados |
|---|---|---|
| Master Summary | `<project>::master_summary` | `project`, `type=master_summary`, `last_updated`, `version` (opcional) |
| Checkpoint de turnos | `<project>::turns::<session_id>::<start>_to_<end>` | `project`, `type=current_session_chunk`, `session_id`, `date`, `start_turn`, `end_turn` |

**Como o projeto é resolvido** (nesta ordem): `--project <nome>` ➔ variável `SESSION_MEMORY_PROJECT` ➔ nome da raiz do repositório Git do diretório atual ➔ nome do diretório atual. O nome é normalizado (minúsculas, kebab-case).

Regras para o agente:
1. **No início da sessão**, determine o projeto (normalmente o nome da raiz do repositório Git em que você está trabalhando) e **passe sempre `--project <nome>` explicitamente** em todos os comandos. Não dependa do diretório atual do shell, que pode mudar durante a sessão.
2. Nunca leia, escreva, consolide ou apague memória de **outro** projeto, a menos que o usuário peça. Se a sessão mudar de projeto, finalize o projeto atual antes de passar ao próximo.
3. O Master Summary de um projeto contém **apenas** fatos daquele projeto. Não misture conteúdo entre projetos.
4. Se `read_memory.py` não encontrar nada para o projeto, rode `--list-projects`: o projeto pode estar salvo com outro nome. Pergunte ao usuário antes de assumir que é um projeto novo.

---

## 🔄 Arquitetura de Memória em 2 Camadas (por projeto)

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               ROLLING 2-LAYER SESSION MEMORY LIFECYCLE (per project)                   │
├─────────────────────────────────────────┬──────────────────────────────────────────────┤
│ 1. LAYER 1: Consolidated Master Summary │ 2. LAYER 2: 20-Turn Incremental Checkpoints  │
│    ('<project>::master_summary')        │    ('<project>::turns::<sid>::N_to_M')       │
├─────────────────────────────────────────┼──────────────────────────────────────────────┤
│ • ONE single, comprehensive text        │ • Every 20 turns during active session, full │
│ • Consolidates ALL prior sessions       │   raw text of turns 1-20, 21-40 is appended  │
│ • Architecture, decisions, releases     │ • Protects against context loss / crashes    │
├─────────────────────────────────────────┴──────────────────────────────────────────────┤
│ 3. SESSION FINALIZATION & MERGE (End of Session):                                      │
│ • Remaining turns (e.g. 41-53) are recorded to complete current session                │
│ • Current session is synthesized & merged with the project's previous Master Summary   │
│ • Result: NEW single '<project>::master_summary' in ChromaDB                           │
│ • Only THIS project's temporary turn chunks are cleaned up                             │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Comandos

### 1. Recuperar contexto (início da sessão) — `read_memory.py`
```bash
python "<skill_dir>/read_memory.py" --project <projeto>
python "<skill_dir>/read_memory.py" --list-projects
```
Se houver checkpoints de turnos sem Master Summary atualizado (sessão anterior não finalizada), consolide-os primeiro com `finalize_session.py`.

### 2. Checkpoint a cada 20 turnos — `save_incremental_turns.py`
```bash
python "<skill_dir>/save_incremental_turns.py" --project <projeto> --start 1 --end 20 --file <turnos.md>
```
Prefira `--file` (arquivo temporário) a `--content` para textos longos ou com aspas. `--session-id` é opcional (padrão: data de hoje).

### 3. Encerrar a sessão e consolidar o Master Summary — `finalize_session.py`
```bash
python "<skill_dir>/finalize_session.py" --project <projeto> --file <sumario_mestre.md> [--version <x.y.z>]
```
O novo sumário **substitui** o anterior: ele precisa conter tudo o que valia no sumário antigo mais o que foi feito na sessão. Leia o sumário atual antes de escrever o novo.

### 4. Migrar uma base antiga (sem projeto) — `migrate_legacy.py`
```bash
python "<skill_dir>/migrate_legacy.py" --source <pasta_chromadb_antiga> --project <projeto> [--dry-run]
```
Copia os registros sem `project` para a base compartilhada, atribuindo-os ao projeto informado. A base de origem não é alterada.

---

## 📝 Estrutura recomendada do Master Summary

1. Visão geral do projeto e da arquitetura
2. Regras e governança específicas do projeto
3. Histórico consolidado de releases e decisões (ADRs)
4. Status atual (versão, testes, pendências)
5. Ponto de partida exato para a próxima sessão

---

## 🔒 Governança

Se a skill [`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md) estiver disponível, siga-a para o ciclo de desenvolvimento. Esta skill cuida apenas da memória.
