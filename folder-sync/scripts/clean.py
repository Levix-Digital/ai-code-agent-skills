import shutil
import argparse
from pathlib import Path

def clean_brain(target_dir: str):
    root_path = Path(target_dir).resolve()
    brain_dir = root_path / ".folder_brain"
    
    if not brain_dir.exists():
        print(f"ℹ️ No `.folder_brain` found in '{root_path.name}'.")
        return
        
    try:
        shutil.rmtree(brain_dir)
        print(f"🧹 `.folder_brain` index removed from '{root_path.name}'.")
    except Exception as e:
        print(f"❌ Error removing `.folder_brain`: {e}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Limpeza do Folder Brain")
    parser.add_argument("--dir", default=".", help="Target directory (default: current)")
    args = parser.parse_args()
    
    clean_brain(args.dir)
