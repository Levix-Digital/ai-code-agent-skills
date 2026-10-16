import os
import sys
import time
import hashlib
import sqlite3
import argparse
from pathlib import Path

# Ensure UTF-8 stdout on Windows
if sys.stdout.encoding != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass

# Add current scripts dir to sys.path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from extractors import extract_text_from_file, chunk_document, is_ignored, SUPPORTED_EXTENSIONS, CLOUD_SHORTCUT_EXTENSIONS

import chromadb
from chromadb.utils import embedding_functions

def calculate_sha256(file_path: Path) -> str:
    h = hashlib.sha256()
    with open(file_path, 'rb') as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def get_db_connection(brain_dir: Path) -> sqlite3.Connection:
    db_path = brain_dir / "sync_state.sqlite"
    conn = sqlite3.connect(db_path)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS file_hashes (
            file_path TEXT PRIMARY KEY,
            relative_path TEXT,
            sha256 TEXT,
            mtime REAL,
            file_size INTEGER,
            chunk_count INTEGER,
            last_synced_at REAL
        )
    """)
    conn.commit()
    return conn

def sync_folder(target_dir: str, force: bool = False):
    root_path = Path(target_dir).resolve()
    if not root_path.exists() or not root_path.is_dir():
        print(f"Erro: O diretório '{target_dir}' não existe ou não é uma pasta válida.")
        return

    brain_dir = root_path / ".folder_brain"
    brain_dir.mkdir(parents=True, exist_ok=True)
    
    chroma_path = brain_dir / "chroma_db"
    
    print(f"\n🧠 [Folder Brain] Iniciando sincronização incremental em:")
    print(f"📂 {root_path}")
    print(f"💾 ChromaDB local: {chroma_path}\n")
    
    # Initialize ChromaDB persistent client
    chroma_client = chromadb.PersistentClient(path=str(chroma_path))
    
    # Default ONNX local embedding function (zero API cost, 100% offline)
    embed_fn = embedding_functions.DefaultEmbeddingFunction()
    
    col_macro = chroma_client.get_or_create_collection(
        name="folder_macro",
        embedding_function=embed_fn,
        metadata={"description": "Document-level overviews"}
    )
    
    col_micro = chroma_client.get_or_create_collection(
        name="folder_micro",
        embedding_function=embed_fn,
        metadata={"description": "Paragraph-level micro chunks"}
    )

    conn = get_db_connection(brain_dir)
    cursor = conn.cursor()
    
    # Load existing state
    cursor.execute("SELECT file_path, sha256, mtime, chunk_count FROM file_hashes")
    known_files = {row[0]: {"sha256": row[1], "mtime": row[2], "chunk_count": row[3]} for row in cursor.fetchall()}
    
    current_files = {}
    cloud_files = []
    
    # Scan filesystem
    for root, dirs, files in os.walk(root_path):
        current_dir = Path(root)
        if is_ignored(current_dir, root_path):
            continue
            
        for f in files:
            file_path = current_dir / f
            if is_ignored(file_path, root_path):
                continue
                
            ext = file_path.suffix.lower()
            if ext in CLOUD_SHORTCUT_EXTENSIONS:
                cloud_files.append(file_path)
                continue
                
            if ext in SUPPORTED_EXTENSIONS:
                try:
                    stat = file_path.stat()
                    current_files[str(file_path)] = {
                        "path": file_path,
                        "mtime": stat.st_mtime,
                        "size": stat.st_size
                    }
                except Exception:
                    pass

    new_count = 0
    modified_count = 0
    unchanged_count = 0
    deleted_count = 0
    total_chunks = 0
    
    start_time = time.time()

    # Process additions and updates
    for f_str, f_info in current_files.items():
        f_path = f_info["path"]
        rel_path = str(f_path.relative_to(root_path)).replace("\\", "/")
        
        is_known = f_str in known_files
        is_modified = False
        
        if is_known and not force:
            # Fast check: if mtime and size match, verify hash only if needed
            known = known_files[f_str]
            if abs(known["mtime"] - f_info["mtime"]) < 0.001:
                unchanged_count += 1
                total_chunks += known.get("chunk_count", 0)
                continue
            
            # If mtime changed, check content hash
            current_hash = calculate_sha256(f_path)
            if current_hash == known["sha256"]:
                # File touched but content identical
                unchanged_count += 1
                total_chunks += known.get("chunk_count", 0)
                cursor.execute("UPDATE file_hashes SET mtime = ? WHERE file_path = ?", (f_info["mtime"], f_str))
                conn.commit()
                continue
            else:
                is_modified = True
        else:
            current_hash = calculate_sha256(f_path)
            
        # Parse and index file
        action_label = "Modificado" if is_modified else ("Reindexando (force)" if force and is_known else "Novo")
        print(f"  ⚡ [{action_label}] {rel_path}")
        
        extracted = extract_text_from_file(f_path)
        if not extracted or extracted.get("type") == "error":
            continue
            
        chunks = chunk_document(extracted)
        
        # If modifying, delete old entries from ChromaDB
        if is_known or is_modified:
            try:
                col_macro.delete(where={"file_path": f_str})
                col_micro.delete(where={"file_path": f_str})
            except Exception:
                pass
                
        # Index Macro (Document Summary)
        doc_preview = extracted["content"][:1500]
        macro_id = f"doc_{hashlib.md5(f_str.encode()).hexdigest()}"
        col_macro.add(
            ids=[macro_id],
            documents=[f"Arquivo: {rel_path}\nTotal de Palavras: {extracted['word_count']}\nPrévia:\n{doc_preview}"],
            metadatas=[{
                "file_path": f_str,
                "relative_path": rel_path,
                "file_name": f_path.name,
                "file_type": f_path.suffix.lower(),
                "word_count": extracted["word_count"]
            }]
        )
        
        # Index Micro (Chunks)
        if chunks:
            chunk_ids = []
            chunk_docs = []
            chunk_metas = []
            
            for idx, ch in enumerate(chunks):
                cid = f"{macro_id}_ch_{idx}"
                heading = ch.get("heading", "Geral")
                page_str = f" (Pág {ch['page']})" if ch.get("page") else ""
                formatted_doc = f"[{rel_path} > {heading}{page_str}]\n{ch['text']}"
                
                chunk_ids.append(cid)
                chunk_docs.append(formatted_doc)
                chunk_metas.append({
                    "file_path": f_str,
                    "relative_path": rel_path,
                    "file_name": f_path.name,
                    "heading": heading,
                    "page": ch.get("page") or 0,
                    "chunk_index": idx,
                    "total_chunks": len(chunks)
                })
                
            col_micro.add(
                ids=chunk_ids,
                documents=chunk_docs,
                metadatas=chunk_metas
            )
            total_chunks += len(chunks)
            
        # Update SQLite state
        cursor.execute("""
            INSERT OR REPLACE INTO file_hashes 
            (file_path, relative_path, sha256, mtime, file_size, chunk_count, last_synced_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        """, (f_str, rel_path, current_hash, f_info["mtime"], f_info["size"], len(chunks), time.time()))
        conn.commit()
        
        if is_modified:
            modified_count += 1
        else:
            new_count += 1

    # Check for deleted files
    for k_path in list(known_files.keys()):
        if k_path not in current_files:
            print(f"  🗑️  [Removido] {k_path}")
            try:
                col_macro.delete(where={"file_path": k_path})
                col_micro.delete(where={"file_path": k_path})
            except Exception:
                pass
            cursor.execute("DELETE FROM file_hashes WHERE file_path = ?", (k_path,))
            conn.commit()
            deleted_count += 1

    conn.close()
    elapsed = round(time.time() - start_time, 2)
    
    print("\n" + "="*60)
    print("✨ Sincronização concluída com sucesso!")
    print(f"⏱️  Tempo decorrido: {elapsed}s")
    print(f"📄 Arquivos novos indexados: {new_count}")
    print(f"📝 Arquivos modificados: {modified_count}")
    print(f"⚡ Arquivos inalterados (cacheados): {unchanged_count}")
    if deleted_count > 0:
        print(f"🗑️  Arquivos removidos do índice: {deleted_count}")
    print(f"🧩 Total de fragmentos (chunks) na base: {total_chunks}")
    print("="*60)
    
    if cloud_files:
        print(f"\n⚠️  Atenção: Foram detectados {len(cloud_files)} atalhos Google Docs (.gdoc) que necessitam de download:")
        for cf in cloud_files:
            print(f"   • {cf.name}")
        print("   👉 Dica: Baixe-os como .docx ou .pdf no Google Drive para incluí-los na próxima sincronização.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Sincronizador Incremental de Pastas com ChromaDB")
    parser.add_argument("--dir", default=".", help="Diretório a ser sincronizado (padrão: atual)")
    parser.add_argument("--force", action="store_true", help="Força a reindexação completa ignorando cache")
    args = parser.parse_args()
    
    sync_folder(args.dir, args.force)
