param(
    [int]$DelaySeconds = 60,
    [switch]$Restart,
    [switch]$Abort,
    [switch]$Force
)

if ($Abort) {
    Write-Host "[ABORT] Abortando desligamento programado..." -ForegroundColor Yellow
    shutdown /a
    Write-Host "[OK] Desligamento abortado com sucesso." -ForegroundColor Green
    exit 0
}

Write-Host "[1/3] Memoria da sessao consolidada no ChromaDB." -ForegroundColor Cyan

Write-Host "[2/3] Identificando aplicativos e janelas abertas do usuario..." -ForegroundColor Cyan

$ignored = @("powershell", "pwsh", "cmd", "conhost", "explorer", "antigravity", "agy", "code", "cursor", "bun", "python")

$processesWithWindows = Get-Process | Where-Object {
    $_.MainWindowHandle -ne 0 -and 
    $ignored -notcontains $_.ProcessName.ToLower()
}

if ($processesWithWindows) {
    Write-Host "[INFO] Solicitando fechamento gracioso para $($processesWithWindows.Count) aplicativo(s)..." -ForegroundColor Yellow
    foreach ($proc in $processesWithWindows) {
        Write-Host "   - Fechando: $($proc.ProcessName) (PID: $($proc.Id))" -ForegroundColor Gray
        try {
            $null = $proc.CloseMainWindow()
        } catch {}
    }
    Start-Sleep -Seconds 3
}

$modeFlag = if ($Restart) { "/r" } else { "/s" }
$actionText = if ($Restart) { "Reinicializacao" } else { "Desligamento" }

Write-Host "[3/3] Disparando $actionText em $DelaySeconds segundos..." -ForegroundColor Green
Write-Host "Dica: Para cancelar digite no terminal: shutdown /a" -ForegroundColor Magenta

if ($Force) {
    shutdown $modeFlag /f /t $DelaySeconds /c "AnyContext Safe Shutdown"
} else {
    shutdown $modeFlag /t $DelaySeconds /c "AnyContext Safe Shutdown"
}
