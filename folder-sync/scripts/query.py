import os
import sys
import json
import argparse
from pathlib import Path

# Ensure UTF-8 stdout on Windows
if sys.stdout.encoding != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass

import chromadb
from chromadb.utils import embedding_functions

def query_brain(target_dir: str, query: str, top_k: int = 6, mode: str = "hybrid", json_output: bool = False):
    root_path = Path(target_dir).resolve()
    brain_dir = root_path / ".folder_brain"
    chroma_path = brain_dir / "chroma_db"
    
    if not chroma_path.exists():
        msg = f"❌ A base de conhecimento da pasta ainda não foi sincronizada.\n👉 Execute `/sync` ou `python sync.py` nesta pasta primeiro."
        if json_output:
            print(json.dumps({"error": msg}))
        else:
            print(msg)
        return

    chroma_client = chromadb.PersistentClient(path=str(chroma_path))
    embed_fn = embedding_functions.DefaultEmbeddingFunction()
    
    results = {
        "query": query,
        "folder": str(root_path),
        "macro_matches": [],
        "micro_chunks": []
    }
    
    # 1. Macro Search (Document level)
    if mode in ["hybrid", "macro"]:
        try:
            col_macro = chroma_client.get_collection("folder_macro", embedding_function=embed_fn)
            macro_res = col_macro.query(
                query_texts=[query],
                n_results=min(3, col_macro.count()) if col_macro.count() > 0 else 1
            )
            if macro_res and macro_res["documents"] and macro_res["documents"][0]:
                for doc, meta, dist in zip(macro_res["documents"][0], macro_res["metadatas"][0], macro_res["distances"][0]):
                    results["macro_matches"].append({
                        "file_name": meta.get("file_name"),
                        "relative_path": meta.get("relative_path"),
                        "file_path": meta.get("file_path"),
                        "word_count": meta.get("word_count"),
                        "distance": round(dist, 4),
                        "summary": doc
                    })
        except Exception as e:
            pass

    # 2. Micro Search (Chunk level)
    if mode in ["hybrid", "micro"]:
        try:
            col_micro = chroma_client.get_collection("folder_micro", embedding_function=embed_fn)
            count = col_micro.count()
            if count > 0:
                micro_res = col_micro.query(
                    query_texts=[query],
                    n_results=min(top_k, count)
                )
                if micro_res and micro_res["documents"] and micro_res["documents"][0]:
                    for doc, meta, dist in zip(micro_res["documents"][0], micro_res["metadatas"][0], micro_res["distances"][0]):
                        results["micro_chunks"].append({
                            "file_name": meta.get("file_name"),
                            "relative_path": meta.get("relative_path"),
                            "file_path": meta.get("file_path"),
                            "heading": meta.get("heading"),
                            "page": meta.get("page"),
                            "distance": round(dist, 4),
                            "content": doc
                        })
        except Exception as e:
            pass

    if json_output:
        print(json.dumps(results, indent=2, ensure_ascii=False))
        return

    # Formatted Markdown Output
    print(f"\n🧠 **Resultados da Busca Semântica na Pasta:** `{root_path.name}`")
    print(f"🔍 **Pergunta/Tema:** *{query}*\n")
    
    if results["macro_matches"]:
        print("### 📚 Documentos Relevantes (Visão Macro)")
        for m in results["macro_matches"]:
            file_link = f"[{m['file_name']}](file:///{m['file_path'].replace('\\', '/')})"
            print(f"- **{file_link}** (`{m['relative_path']}`) | Score: {1 - m['distance']:.2f}")
        print()

    if results["micro_chunks"]:
        print("### 🧩 Trechos Específicos Encontrados (Visão Micro)")
        for idx, c in enumerate(results["micro_chunks"], 1):
            file_link = f"[{c['file_name']}](file:///{c['file_path'].replace('\\', '/')})"
            heading_info = f" > {c['heading']}" if c.get('heading') else ""
            page_info = f" (Pág {c['page']})" if c.get('page') else ""
            score = 1 - c['distance']
            
            print(f"**[{idx}] {file_link}{heading_info}{page_info}** *(Relevância: {score:.2%})*")
            print("```text")
            print(c['content'])
            print("```\n")
    else:
        print("Nenhum trecho relevante encontrado para esta consulta.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Consulta Semântica ao Folder Brain")
    parser.add_argument("--dir", default=".", help="Diretório alvo (padrão: atual)")
    parser.add_argument("--query", required=True, help="Texto da consulta ou pergunta")
    parser.add_argument("--top-k", type=int, default=6, help="Número de chunks a retornar")
    parser.add_argument("--mode", choices=["hybrid", "macro", "micro"], default="hybrid")
    parser.add_argument("--json", action="store_true", help="Retornar saída em formato JSON")
    args = parser.parse_args()
    
    query_brain(args.dir, args.query, args.top_k, args.mode, args.json)
