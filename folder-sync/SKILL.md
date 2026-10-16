---
name: folder-sync
description: Sincronização inteligente e RAG vetorial incremental para qualquer pasta/workspace usando ChromaDB. Permite indexar recursivamente documentos (.docx, .pdf, .md, .txt, código) via comando /sync e conversar com a base de conhecimento da pasta via /folder.
---

# Folder Sync & Brain (RAG Incremental para Pastas)

> **Caminhos**: `<skills_root>` é a pasta que contém esta skill (o diretório pai da pasta deste `SKILL.md`). Use o caminho absoluto real ao executar os comandos.

Esta skill permite ao agente indexar qualquer pasta local recursivamente em um banco vetorial **ChromaDB** local de forma **incremental** (via hash SHA-256) e realizar buscas semânticas de alta precisão (Macro e Micro) sem estourar o limite de tokens da conversa.

## Comandos e Gatilhos

### 1. `/sync` (Sincronização Incremental do Diretório Atual)
Quando o usuário digitar `/sync` ou pedir para sincronizar/indexar a pasta atual:
1. Execute o script de sincronização:
   ```bash
   python "<skills_root>/folder-sync/scripts/sync.py" --dir "."
   ```
2. O script irá:
   - Varrer recursivamente todos os arquivos suportados (`.docx`, `.pdf`, `.md`, `.txt`, `.json`, `.csv`, `.py`, `.ts`, etc.).
   - Ignorar automaticamente `.git`, `node_modules`, `.venv`, `.folder_brain` e binários não textuais.
   - Verificar o hash SHA-256 de cada arquivo contra o cache local. Se o arquivo não mudou, **pula em milissegundos**.
   - Se o arquivo foi modificado/criado, extrai o texto, gera chunks inteligentes com overlap e atualiza o ChromaDB local (`.folder_brain/chroma_db`).
   - Identificar arquivos `.gdoc` e avisar se houver documentos na nuvem pendentes de exportação.
3. Apresente um resumo claro com: total de arquivos indexados, novos/modificados, inalterados e chunks gerados.

---

### 2. `/folder` (Ativação e Consulta ao Contexto da Pasta)
Quando o usuário digitar `/folder` (ou fizer perguntas sobre o conteúdo da pasta após indexá-la):
1. Execute a busca semântica na base ChromaDB da pasta:
   ```bash
   python "<skills_root>/folder-sync/scripts/query.py" --dir "." --query "<pergunta ou tema do usuário>" --top-k 6
   ```
2. O script retorna:
   - **Visão Macro**: Resumo dos documentos relevantes encontrados.
   - **Trechos Micro (Chunks)**: Parágrafos exatos com caminhos de arquivo, número de página/seção e score de similaridade.
3. Utilize os trechos retornados para responder com máxima precisão, citando os arquivos como links markdown clicáveis (`[nome_do_arquivo](file:///caminho_absoluto)`).

---

### 3. Consultas Estatísticas ou Limpeza (`/folder-stats`, `/folder-clean`)
- **Ver estatísticas do índice:**
  ```bash
  python "<skills_root>/folder-sync/scripts/stats.py" --dir "."
  ```
- **Resetar / Reindexar do zero:**
  ```bash
  python "<skills_root>/folder-sync/scripts/sync.py" --dir "." --force
  ```

---

## Estrutura da Skill
- `scripts/sync.py`: Motor de indexação incremental e chunking hierárquico.
- `scripts/query.py`: Motor de busca semântica com ranking e formatação.
- `scripts/stats.py`: Inspetor de estatísticas da base ChromaDB local.
- `scripts/extractors.py`: Extratores especializados para cada formato de arquivo (.docx, .pdf, .txt, .md, código).
