<#
.SYNOPSIS
  Links every skill in a source folder into the skills folder of each coding agent.

.DESCRIPTION
  A skill is any top-level folder that contains a SKILL.md. Each one is linked
  (directory junction, no admin rights needed) into every target folder, so all
  agents read the same files.

  Existing entries with the same name that are not links to the source are
  reported and skipped, never overwritten.

.EXAMPLE
  .\install.ps1
  .\install.ps1 -Source "$HOME\.ai-code-agent-skills-personal"
  .\install.ps1 -Targets "$HOME\.claude\skills","$HOME\.copilot\skills"
#>
param(
    [string]$Source = $PSScriptRoot,
    [string[]]$Targets = @(
        "$HOME\.claude\skills",          # Claude Code
        "$HOME\.agents\skills",          # Cross-agent standard location (Codex and others)
        "$HOME\.gemini\config\skills"    # Gemini
    ),
    [string[]]$Exclude = @()
)

$repo = (Resolve-Path $Source).Path.TrimEnd('\')
$skills = Get-ChildItem -Path $repo -Directory |
    Where-Object { (Test-Path (Join-Path $_.FullName 'SKILL.md')) -and ($Exclude -notcontains $_.Name) }

function Get-LinkTarget([string]$path) {
    $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    if ($item -and $item.LinkType) { return ([string]$item.Target).TrimEnd('\') }
    return $null
}

foreach ($target in $Targets) {
    Write-Host "`n== $target"
    if (Get-LinkTarget $target) {
        Write-Warning "   '$target' is itself a link; replace it with a real folder to link skills individually. Skipped."
        continue
    }
    New-Item -ItemType Directory -Force -Path $target | Out-Null

    foreach ($skill in $skills) {
        $dest = Join-Path $target $skill.Name
        $current = Get-LinkTarget $dest
        if ($current -and ($current -eq $skill.FullName)) {
            Write-Host "   = $($skill.Name)"
        } elseif (Test-Path -LiteralPath $dest) {
            Write-Warning "   ! $($skill.Name): '$dest' already exists and is not a link to '$repo', skipped"
        } else {
            New-Item -ItemType Junction -Path $dest -Target $skill.FullName | Out-Null
            Write-Host "   + $($skill.Name)"
        }
    }
}
