# Runs the dev-cycle binary of this skills version (the commands translated from the Python
# scripts, IMP-005) from PowerShell; sh shells (Linux, macOS, Git Bash) use dev-cycle.
#
# The binary comes from, in order: DEV_CYCLE_BIN; the cache; the engine-v<version> release
# (checked against its SHA256SUMS); a build with cargo when this folder is inside a checkout.
$ErrorActionPreference = 'Stop'

$Repo = 'Levix-Digital/ai-code-agent-skills'
$Version = (Get-Content -Raw (Join-Path $PSScriptRoot 'engine-version')).Trim()

# The commands read their sibling skills' files (SKILL.md, styles) from this skills folder.
$env:DEV_CYCLE_SKILLS_ROOT = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ($env:DEV_CYCLE_BIN) {
    & $env:DEV_CYCLE_BIN @args
    exit $LASTEXITCODE
}

$onWindows = ($PSVersionTable.PSVersion.Major -lt 6) -or $IsWindows
$arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
    'X64' { 'x86_64' }
    'Arm64' { 'aarch64' }
    default { $null }
}
$exe = ''
if ($onWindows) { $os = 'pc-windows-msvc'; $exe = '.exe' }
elseif ($IsMacOS) { $os = 'apple-darwin' }
else { $os = 'unknown-linux-musl' }
$target = if ($arch) { "$arch-$os" } else { $null }

$cacheRoot = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA }
             elseif ($env:XDG_CACHE_HOME) { $env:XDG_CACHE_HOME }
             else { Join-Path $HOME '.cache' }
$cache = Join-Path (Join-Path $cacheRoot 'dev-cycle') "engine-v$Version"
$bin = Join-Path $cache "dev-cycle$exe"

if (-not (Test-Path $bin) -and $target) {
    $url = "https://github.com/$Repo/releases/download/engine-v$Version"
    $asset = "dev-cycle-$target$exe"
    $tmp = "$cache.tmp.$PID"
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -UseBasicParsing -Uri "$url/$asset" -OutFile (Join-Path $tmp $asset)
        Invoke-WebRequest -UseBasicParsing -Uri "$url/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS')
        $line = Get-Content (Join-Path $tmp 'SHA256SUMS') | Where-Object { $_ -match " $([regex]::Escape($asset))$" } | Select-Object -First 1
        $expected = if ($line) { ($line -split ' ')[0] } else { '' }
        $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $asset)).Hash.ToLower()
        if ($expected -and $expected -eq $actual) {
            New-Item -ItemType Directory -Force -Path $cache | Out-Null
            Move-Item -Force (Join-Path $tmp $asset) $bin
            if (-not $onWindows) { chmod +x $bin }
        } else {
            Write-Error "dev-cycle: checksum mismatch for $asset of engine-v$Version; not used" -ErrorAction Continue
        }
    } catch {
        # No release for this version or this system yet: fall back to a build below.
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

if (-not (Test-Path $bin)) {
    $root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    if ((Test-Path (Join-Path $root 'Cargo.toml')) -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
        & cargo build --quiet --release --locked -p dev-cycle --manifest-path (Join-Path $root 'Cargo.toml') 2>&1 | ForEach-Object { [Console]::Error.WriteLine($_) }
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $bin = Join-Path $root "target/release/dev-cycle$exe"
    } else {
        [Console]::Error.WriteLine("dev-cycle: no binary of engine-v$Version for this system ($target).")
        [Console]::Error.WriteLine("Download it from https://github.com/$Repo/releases/tag/engine-v$Version, or install Rust and run this again from a checkout.")
        exit 127
    }
}

& $bin @args
exit $LASTEXITCODE
