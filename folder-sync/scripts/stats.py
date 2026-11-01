import os
import sys
import sqlite3
import argparse
from datetime import datetime
from pathlib import Path
# Ensure UTF-8 stdout on Windows
if sys.stdout.encoding != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass

def show_stats(target_dir: str):
    root_path = Path(target_dir).resolve()
    brain_dir = root_path / ".folder_brain"
    db_path = brain_dir / "sync_state.sqlite"
    
    if not brain_dir.exists() or not db_path.exists():
        print(f"ℹ️ Folder '{root_path.name}' has no synced `.folder_brain` index yet.")
        return

    conn = sqlite3.connect(db_path)
    c = conn.cursor()
    
    c.execute("SELECT COUNT(*), SUM(chunk_count), SUM(file_size), MAX(last_synced_at) FROM file_hashes")
    total_files, total_chunks, total_size, last_sync = c.fetchone()
    
    c.execute("SELECT relative_path, chunk_count, last_synced_at FROM file_hashes ORDER BY chunk_count DESC LIMIT 10")
    top_files = c.fetchall()
    conn.close()

    size_mb = round((total_size or 0) / (1024 * 1024), 2)
    last_sync_str = datetime.fromtimestamp(last_sync).strftime("%d/%m/%Y %H:%M:%S") if last_sync else "Nunca"

    print(f"\n📊 **Folder Brain Statistics:** `{root_path.name}`")
    print(f"📁 Path: `{root_path}`")
    print(f"📄 Total indexed files: {total_files or 0}")
    print(f"🧩 Total vector chunks: {total_chunks or 0}")
    print(f"💾 Volume total de dados: {size_mb} MB")
    print(f"🕒 Last sync: {last_sync_str}\n")
    
    if top_files:
        print("🔝 **Maiores documentos indexados:**")
        for rel_path, chunks, synced in top_files:
            print(f"- `{rel_path}` ({chunks} chunks)")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Folder Brain statistics")
    parser.add_argument("--dir", default=".", help="Target directory (default: current)")
    args = parser.parse_args()
    
    show_stats(args.dir)
