# Time the PowerShell capture hook: what one command costs with and without it.
#
#   pwsh -NoProfile -File benchmarks/hook_overhead.ps1 [-Runs 60] [-Cdg path\to\cdg.exe]
#
# Runs at BelowNormal priority with captures written to a temporary CODIGEST_HOME.
# Prints median and p95 in milliseconds for:
#   wrap      the line rewrite PSConsoleHostReadLine does before a command runs
#             (skip regexes and the parser check), per typed line
#   command   one short command run as `. __cdg { ... }` against `. { ... }`; the
#             difference includes buffering the output and starting `cdg term ingest`
#   lines     20,000 output lines through the hook against without it
#   ingest    `cdg term ingest` itself, which runs in the background after the prompt
#             returns, so it costs CPU but not prompt latency
param(
    [int]$Runs = 60,
    [string]$Cdg = (Get-Command cdg -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
)

$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'
$env:CODIGEST_HOME = Join-Path ([IO.Path]::GetTempPath()) "codigest-bench-$PID"
New-Item -ItemType Directory -Force $env:CODIGEST_HOME | Out-Null

$hook = Join-Path $PSScriptRoot '..\src\codigest\shell\hook.ps1'
. $hook
$global:__cdgExe = $Cdg

# PSConsoleHostReadLine sets the typed line before every command, and __cdg clears it after.
function Typed { $global:__cdgLine = 'bench' }

# Each hooked command starts `cdg term ingest` in the background. Wait for those between
# measurements (outside the timed part) so they neither pile up in memory nor overlap the
# next timing.
function Settle {
    while (Get-CimInstance Win32_Process -Filter "ParentProcessId=$PID" | Where-Object Name -ne 'conhost.exe') {
        Start-Sleep -Milliseconds 20
    }
}

function Stat([double[]]$ms) {
    $s = $ms | Sort-Object
    $median = if ($s.Count % 2) { $s[[int][Math]::Floor($s.Count / 2)] } else { ($s[$s.Count / 2 - 1] + $s[$s.Count / 2]) / 2 }
    $p95 = $s[[int][Math]::Ceiling(0.95 * $s.Count) - 1]
    [pscustomobject]@{ median = [Math]::Round($median, 2); p95 = [Math]::Round($p95, 2); n = $s.Count }
}

function Time([scriptblock]$block) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $block
    $sw.Stop()
    $sw.Elapsed.TotalMilliseconds
}

# The rewrite PSConsoleHostReadLine applies to a typed line, as in hook.ps1.
function Wrap([string]$line) {
    $wrap = $line.Trim() -and $line -notmatch '^\s' -and $line -notmatch $global:__cdgSkip -and
            -not ($env:CODIGEST_CAP_SKIP -and $line -match $env:CODIGEST_CAP_SKIP)
    if ($wrap) {
        $tokens = $null; $errs = $null
        foreach ($w in ". __cdg { $line }", ". __cdg { $line`n}") {
            [void][Management.Automation.Language.Parser]::ParseInput($w, [ref]$tokens, [ref]$errs)
            if (-not $errs) { return $w }
        }
    }
    $line
}

$line = "Get-ChildItem -Path . -Filter *.py | Select-Object -First 3"
$results = [ordered]@{}

# Warm up the parser, the pipeline and process start once.
$null = Wrap $line
Typed
$null = . __cdg { 'warm' }
$null = . { 'warm' }
Settle

$results['wrap'] = Stat (1..$Runs | ForEach-Object { Time { $null = Wrap $line } })

# Alternate plain and hooked runs so drift on a shared machine hits both alike.
$plain = [Collections.Generic.List[double]]::new()
$hooked = [Collections.Generic.List[double]]::new()
foreach ($i in 1..$Runs) {
    $plain.Add((Time { $null = . { Write-Output "run $i" } }))
    Typed
    $hooked.Add((Time { $null = . __cdg { Write-Output "run $i" } }))
    Settle
}
$results['command plain'] = Stat $plain
$results['command hooked'] = Stat $hooked
$results['command added'] = Stat (0..($Runs - 1) | ForEach-Object { $hooked[$_] - $plain[$_] })

$lineRuns = [Math]::Max(5, [int]($Runs / 6))
$plain = [Collections.Generic.List[double]]::new()
$hooked = [Collections.Generic.List[double]]::new()
foreach ($i in 1..$lineRuns) {
    $plain.Add((Time { $null = . { 1..20000 | ForEach-Object { "line $_" } } }))
    Typed
    $hooked.Add((Time { $null = . __cdg { 1..20000 | ForEach-Object { "line $_" } } }))
    Settle
}
$results['20k lines plain'] = Stat $plain
$results['20k lines hooked'] = Stat $hooked

$json = [Text.Encoding]::UTF8.GetBytes((@{
    command = 'pytest -q'; output = ("x" * 2000); exit_code = 1; cwd = $PWD.Path
    shell = 'powershell'; started_at = (Get-Date).ToString('yyyy-MM-ddTHH:mm:sszzz'); duration_ms = 10
} | ConvertTo-Json -Compress))
$ingest = foreach ($i in 1..[Math]::Max(10, [int]($Runs / 3))) {
    Time {
        $psi = [Diagnostics.ProcessStartInfo]::new($Cdg, 'term ingest')
        $psi.UseShellExecute = $false
        $psi.RedirectStandardInput = $true
        $p = [Diagnostics.Process]::Start($psi)
        $p.StandardInput.BaseStream.Write($json, 0, $json.Length)
        $p.StandardInput.Close()
        $p.WaitForExit()
    }
}
$results['ingest (background)'] = Stat $ingest

Settle
$history = Join-Path $env:CODIGEST_HOME 'term\history.jsonl'
$stored = if (Test-Path $history) { @(Get-Content $history).Count } else { 0 }
Remove-Item -Recurse -Force $env:CODIGEST_HOME -ErrorAction Ignore

"PowerShell $($PSVersionTable.PSVersion), PSReadLine $((Get-Module PSReadLine).Version), cdg: $Cdg"
"runs: $Runs ($lineRuns for 20k lines), priority BelowNormal; entries stored by the hooked runs and ingest: $stored"
''
'| measurement | median ms | p95 ms | n |'
'|---|---:|---:|---:|'
foreach ($k in $results.Keys) {
    $r = $results[$k]
    "| $k | $($r.median) | $($r.p95) | $($r.n) |"
}
