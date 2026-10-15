# Codigest terminal capture hook for PowerShell. Installed by `cdg term install`.
#
# On Enter, the accepted line is rewritten to `. __cdg { <line> }`. Dot-sourcing
# runs it in the caller's scope, so `cd`, variable assignments and function
# definitions behave exactly as typed. Output is teed into a buffer and handed
# to `cdg term ingest` in a background process (redaction happens there,
# before anything touches disk).
#
# Opt out for one command: start the line with a space.
# Opt out entirely: $env:CODIGEST_NOCAP = 1

if ($env:CODIGEST_NOCAP) { return }
$global:__cdgExe = (Get-Command cdg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if (-not $global:__cdgExe) { return }

function global:__cdg {
    # Advanced function so a failed command can surface as `$? -eq $false` for prompt themes.
    [CmdletBinding()]
    param([scriptblock]$__cdgBlock)
    # Dot-sourced: these locals live in the caller's scope, hence the prefix and cleanup.
    $__cdgLine = $global:__cdgLine
    $__cdgErr = $global:Error.Count
    $__cdgPrevExit = $global:LASTEXITCODE
    $global:LASTEXITCODE = $null
    $__cdgUnbuf = $env:PYTHONUNBUFFERED
    $env:PYTHONUNBUFFERED = '1'  # piped Python would otherwise block-buffer live output
    $__cdgStart = Get-Date
    $__cdgOut = $null
    try {
        . $__cdgBlock *>&1 | Tee-Object -Variable __cdgOut
    }
    finally {
        $env:PYTHONUNBUFFERED = $__cdgUnbuf
        $__cdgExit = if ($null -ne $global:LASTEXITCODE) { $global:LASTEXITCODE }
                     elseif ($global:Error.Count -gt $__cdgErr) { 1 } else { 0 }
        if ($null -eq $global:LASTEXITCODE) { $global:LASTEXITCODE = $__cdgPrevExit }
        try {
            $__cdgJson = @{
                command     = $__cdgLine
                output      = ($__cdgOut | Out-String)
                exit_code   = $__cdgExit
                cwd         = $PWD.ProviderPath
                shell       = 'powershell'
                started_at  = $__cdgStart.ToString('yyyy-MM-ddTHH:mm:sszzz')
                duration_ms = [int]((Get-Date) - $__cdgStart).TotalMilliseconds
            } | ConvertTo-Json -Compress
            $__cdgBytes = [Text.Encoding]::UTF8.GetBytes($__cdgJson)
            $__cdgPsi = New-Object Diagnostics.ProcessStartInfo $global:__cdgExe, 'term ingest'
            $__cdgPsi.UseShellExecute = $false
            $__cdgPsi.CreateNoWindow = $true
            $__cdgPsi.RedirectStandardInput = $true
            $__cdgPsi.RedirectStandardOutput = $true
            $__cdgPsi.RedirectStandardError = $true
            $__cdgProc = [Diagnostics.Process]::Start($__cdgPsi)
            $__cdgProc.StandardInput.BaseStream.Write($__cdgBytes, 0, $__cdgBytes.Length)
            $__cdgProc.StandardInput.Close()
        }
        catch { }
        if ($__cdgExit) {
            # Sets $? to false without printing anything or touching $Error.
            $__cdgPref = $ErrorActionPreference
            $ErrorActionPreference = 'Ignore'
            $PSCmdlet.WriteError([Management.Automation.ErrorRecord]::new(
                [Exception]::new("exit $__cdgExit"), 'CodigestExit', 'NotSpecified', $null))
            $ErrorActionPreference = $__cdgPref
        }
        Remove-Variable __cdgPref, __cdgBlock, __cdgLine, __cdgErr, __cdgPrevExit, __cdgUnbuf, __cdgStart, __cdgOut, __cdgExit, __cdgJson, __cdgBytes, __cdgPsi, __cdgProc -ErrorAction Ignore
    }
}

# Windows PowerShell 5.1 may not have PSReadLine loaded yet while the profile runs.
if (-not (Get-Module PSReadLine) -and $Host.Name -eq 'ConsoleHost') {
    Import-Module PSReadLine -ErrorAction Ignore
}
if (-not (Get-Module PSReadLine)) { return }

# ponytail: denylist heuristic for programs that need a real TTY (editors, pagers,
# REPLs, ssh). Extend with $env:CODIGEST_CAP_SKIP (a regex) when one slips through.
$global:__cdgSkip = '^\s*(vim?|nvim|nano|emacs|less|more|ssh|htop|btop|top|fzf|lazygit|claude|codex|wsl|exit)(\.exe)?\b' +
    '|^\s*(cdg|codigest)(\.exe)?\s+(term|mcp)\b' +
    '|^\s*(python3?|py|ipython|node|pwsh|powershell|bash|cmd)(\.exe)?\s*$' +
    '|^\s*git\s+(commit(?!.*\s-\w*[mF])|rebase\s+-i|add\s+-[pi]|mergetool)'

Set-PSReadLineKeyHandler -Key Enter -BriefDescription CodigestAcceptLine -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $wrap = $line.Trim() -and $line -notmatch '^\s' -and $line -notmatch $global:__cdgSkip -and
            -not ($env:CODIGEST_CAP_SKIP -and $line -match $env:CODIGEST_CAP_SKIP) -and -not $env:CODIGEST_NOCAP
    if ($wrap) {
        $tokens = $null; $errs = $null
        [void][Management.Automation.Language.Parser]::ParseInput($line, [ref]$tokens, [ref]$errs)
        # Incomplete input (open brace etc.) goes through untouched to the continuation prompt.
        if (-not $errs) {
            # Single-line form first; the newline form survives a trailing `# comment`.
            foreach ($w in ". __cdg { $line }", ". __cdg { $line`n}") {
                [void][Management.Automation.Language.Parser]::ParseInput($w, [ref]$tokens, [ref]$errs)
                if (-not $errs) {
                    $global:__cdgLine = $line
                    [Microsoft.PowerShell.PSConsoleReadLine]::AddToHistory($line)
                    [Microsoft.PowerShell.PSConsoleReadLine]::Replace(0, $line.Length, $w)
                    break
                }
            }
        }
    }
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
}

# Keep the wrapper out of history; the original line was added above.
$global:__cdgPrevHistHandler = (Get-PSReadLineOption).AddToHistoryHandler
Set-PSReadLineOption -AddToHistoryHandler {
    param($l)
    if ($l -like '. __cdg {*') { return $false }
    # The default handler is a .NET Func delegate, not a scriptblock: .Invoke() covers both.
    if ($global:__cdgPrevHistHandler) { return $global:__cdgPrevHistHandler.Invoke($l) }
    return $true
}

# Lets `cdg term` tell whether this shell is hooked.
$env:CODIGEST_HOOK = '1'
