import os
import sys
import subprocess
import time

def safe_shutdown(delay_seconds=30, restart=False, force=False):
    print("=" * 60)
    print("🛑 AnyContext Safe Shutdown Protocol")
    print("=" * 60)

    # 1. Session memory is consolidated by the agent (session-memory/finalize_session.py) before this script runs
    print("\n🧠 [1/3] A memoria da sessao deve ter sido consolidada pelo agente (session-memory/finalize_session.py).")

    # 2. Gracefully request user window close
    print("\n🔍 [2/3] Identificando aplicativos e janelas abertas do usuario...")
    ps_cmd = (
        '$ignored = @("powershell", "pwsh", "cmd", "conhost", "explorer", "antigravity", "agy", "code", "cursor", "bun", "python"); '
        'Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $ignored -notcontains $_.ProcessName.ToLower() } | '
        'ForEach-Object { try { Write-Host "   • Fechando: " $_.ProcessName; $_.CloseMainWindow() } catch {} }'
    )
    try:
        subprocess.run(["powershell", "-NoProfile", "-Command", ps_cmd], capture_output=True, text=True)
        time.sleep(3)
    except Exception as e:
        print(f"⚠ Aviso ao fechar janelas: {e}")

    # 3. Trigger Windows shutdown
    mode_flag = "/r" if restart else "/s"
    action_text = "Reinicializacao" if restart else "Desligamento"
    print(f"\n⚡ [3/3] Disparando {action_text} em {delay_seconds} segundos...")
    print("💡 Dica: Para cancelar o desligamento, digite no terminal: shutdown /a\n")

    cmd = ["shutdown", mode_flag, "/t", str(delay_seconds), "/c", "AnyContext Safe Shutdown"]
    if force:
        cmd.insert(2, "/f")
    subprocess.run(cmd)

if __name__ == "__main__":
    delay = 30
    if len(sys.argv) > 1:
        try:
            delay = int(sys.argv[1])
        except ValueError:
            pass
    safe_shutdown(delay_seconds=delay)
