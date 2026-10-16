<#
.SYNOPSIS
  Links every skill in this repository into the skills folder of each coding agent.

.DESCRIPTION
  A skill is any top-level folder that contains a SKILL.md. Each one is linked
  (directory junction, no admin rights needed) into every target folder, so all
  agents read the same files from this repository.

  Targets that already point at this repository as a whole are left untouched.
  Existing entries with the same name that are not links to this repository are
  reported and skipped, never overwritten.

.EXAMPLE
  .\install.ps1
  .\install.ps1 -Targets "$HOME\.claude\skills","$HOME\.agents\skills","$HOME\.copilot\skills"
  .\install.ps1 -Exclude assistente-diario
#>
param(
    [string[]]$Targets = @(
        "$HOME\.claude\skills",   # Claude Code
        "$HOME\.agents\skills"    # Cross-agent standard location (Codex, Gemini CLI and others)
    ),
    [string[]]$Exclude = @()
)

$repo = (Resolve-Path $PSScriptRoot).Path.TrimEnd('\')
$skills = Get-ChildItem -Path $repo -Directory |
    Where-Object { (Test-Path (Join-Path $_.FullName 'SKILL.md')) -and ($Exclude -notcontains $_.Name) }

function Get-LinkTarget([string]$path) {
    $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    if ($item -and $item.LinkType) { return ([string]$item.Target).TrimEnd('\') }
    return $null
}

foreach ($target in $Targets) {
    Write-Host "`n== $target"
    $whole = Get-LinkTarget $target
    if ($whole -and ((Resolve-Path $whole).Path.TrimEnd('\') -eq $repo)) {
        Write-Host "   already links to the whole repository, nothing to do"
        continue
    }
    New-Item -ItemType Directory -Force -Path $target | Out-Null

    foreach ($skill in $skills) {
        $dest = Join-Path $target $skill.Name
        $current = Get-LinkTarget $dest
        if ($current -and ($current -eq $skill.FullName)) {
            Write-Host "   = $($skill.Name)"
        } elseif (Test-Path -LiteralPath $dest) {
            Write-Warning "   ! $($skill.Name): '$dest' already exists and is not a link to this repository, skipped"
        } else {
            New-Item -ItemType Junction -Path $dest -Target $skill.FullName | Out-Null
            Write-Host "   + $($skill.Name)"
        }
    }
}
