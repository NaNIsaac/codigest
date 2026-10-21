# Installs cdg, the Codigest binary, from a GitHub release.
#
#   irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
#
# CDG_VERSION=0.12.1 picks a release instead of the latest one. CDG_INSTALL_DIR picks the
# folder; the default one is added to the user PATH, a chosen one is left to you.
# Running it again replaces the installed binary.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'NaNIsaac/codigest'
# Only an x64 build exists; Windows on ARM runs it under emulation.
$target = 'x86_64-pc-windows-msvc'

if ($env:CDG_VERSION) {
    $tag = 'rust-v' + ($env:CDG_VERSION -replace '^(rust-)?v', '')
    $release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/tags/$tag"
} else {
    # not releases/latest: a GitHub release for a Python v* tag would take that spot
    $release = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases?per_page=30") |
        Where-Object { -not $_.draft -and -not $_.prerelease -and $_.tag_name -like 'rust-v*' } |
        Select-Object -First 1
    if (-not $release) { throw "No rust-v* release found in $repo." }
}
$asset = $release.assets | Where-Object { $_.name -like "codigest-*-$target.zip" } | Select-Object -First 1
if (-not $asset) {
    throw "Release $($release.tag_name) has no $target archive."
}

$custom = [bool]$env:CDG_INSTALL_DIR
$dir = if ($custom) { $env:CDG_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\codigest' }

$work = Join-Path ([IO.Path]::GetTempPath()) ("cdg-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $work | Out-Null
try {
    $zip = Join-Path $work $asset.name
    Write-Host "Downloading $($asset.name)"
    Invoke-WebRequest $asset.browser_download_url -OutFile $zip
    # GitHub records each asset's SHA-256 as "sha256:<hex>"
    if ("$($asset.digest)" -notmatch '^sha256:([0-9a-f]{64})$') {
        throw "Release $($release.tag_name) has no SHA-256 digest for $($asset.name)."
    }
    $fs = [IO.File]::OpenRead($zip)
    try { $got = -join ([Security.Cryptography.SHA256]::Create().ComputeHash($fs) | ForEach-Object { $_.ToString('x2') }) } finally { $fs.Dispose() }
    if ($got -ne $Matches[1]) { throw "Checksum mismatch for $($asset.name): expected $($Matches[1]), got $got." }
    Expand-Archive $zip $work
    $exe = Get-ChildItem $work -Recurse -Filter cdg.exe | Select-Object -First 1
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item $exe.FullName (Join-Path $dir 'cdg.exe') -Force
} finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$onPath = ($userPath -split ';') -contains $dir
if (-not $custom -and -not $onPath) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir").TrimStart(';'), 'User')
    Write-Host "Added $dir to your user PATH."
}
if (-not (($env:Path -split ';') -contains $dir)) {
    $env:Path = "$env:Path;$dir"
}

Write-Host "Installed $(& (Join-Path $dir 'cdg.exe') --version) to $dir"
# The Python package (uv tool, pipx) also provides cdg; whichever is first on PATH runs.
$first = Get-Command cdg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if ($first -and -not ((Split-Path $first.Source) -ieq $dir.TrimEnd('\'))) {
    Write-Warning "cdg in new shells is $($first.Source) ($(& $first.Source --version)), not this install. Remove one (e.g. 'uv tool uninstall codigest') or put $dir earlier on PATH."
}
if ($custom -and -not $onPath) {
    Write-Host "Add $dir to PATH to run cdg from anywhere."
}
Write-Host "Next: cdg term install   (captures command output in new shells)"
