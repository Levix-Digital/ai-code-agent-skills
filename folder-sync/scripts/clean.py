import shutil
import argparse
from pathlib import Path

def clean_brain(target_dir: str):
    root_path = Path(target_dir).resolve()
    brain_dir = root_path / ".folder_brain"
    
    if not brain_dir.exists():
        print(f"ℹ️ Nenhum `.folder_brain` encontrado em '{root_path.name}'.")
        return
        
    try:
        shutil.rmtree(brain_dir)
        print(f"🧹 Base `.folder_brain` removida com sucesso de '{root_path.name}'.")
    except Exception as e:
        print(f"❌ Erro ao remover `.folder_brain`: {e}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Limpeza do Folder Brain")
    parser.add_argument("--dir", default=".", help="Diretório alvo (padrão: atual)")
    args = parser.parse_args()
    
    clean_brain(args.dir)
