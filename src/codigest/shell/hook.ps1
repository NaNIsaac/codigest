# Codigest terminal capture hook for PowerShell. Installed by `cdg term install`.
#
# The line you type is run as `. __cdg { <line> }`. The wrapping happens in
# PSConsoleHostReadLine, after PSReadLine has returned the line, so the screen and
# the history keep the line as typed. Dot-sourcing runs it in the caller's scope,
# so `cd`, variable assignments and function definitions behave exactly as typed.
# Output is teed into a buffer and handed to `cdg term ingest` in a background
# process (redaction happens there, before anything touches disk).
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
    # Left as is so the line can read it; a native command stores a new boxed int,
    # so a reference change means one ran, even with the same exit code.
    $__cdgPrevExit = $global:LASTEXITCODE
    $__cdgUnbuf = $env:PYTHONUNBUFFERED
    $env:PYTHONUNBUFFERED = '1'  # piped Python would otherwise block-buffer live output
    $__cdgStart = Get-Date
    $__cdgOut = $null
    try {
        . $__cdgBlock *>&1 | Tee-Object -Variable __cdgOut
    }
    finally {
        $env:PYTHONUNBUFFERED = $__cdgUnbuf
        $__cdgExit = if (-not [object]::ReferenceEquals($global:LASTEXITCODE, $__cdgPrevExit)) { $global:LASTEXITCODE }
                     elseif ($global:Error.Count -gt $__cdgErr) { 1 } else { 0 }
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

# Denylist heuristic for programs that need a real TTY (editors, pagers,
# REPLs, ssh). Extend with $env:CODIGEST_CAP_SKIP (a regex) when one slips through.
$global:__cdgSkip = '^\s*(vim?|nvim|nano|emacs|less|more|ssh|htop|btop|top|fzf|lazygit|claude|codex|wsl|exit)(\.exe)?\b' +
    # Full-screen programs: through the capture pipe they get no terminal and hang.
    '|^\s*(herdr|tmux|screen|zellij|k9s|lazydocker|gitui|tig|yazi|ranger|nnn|mc|vifm|broot|hx|helix|micro|ncdu|btm|glances|nvtop|man)(\.exe)?\b' +
    '|^\s*(cdg|codigest)(\.exe)?\s+(term|mcp)\b' +
    '|^\s*(python3?|py|ipython|node|pwsh|powershell|bash|cmd)(\.exe)?\s*$' +
    '|^\s*git\s+(commit(?!.*\s-\w*[mF])|rebase\s+-i|add\s+-[pi]|mergetool)'

# Replaces PSReadLine's own PSConsoleHostReadLine, which the host calls for every prompt.
# PSReadLine draws and stores the line as typed; only the string returned to the engine
# is wrapped, so nothing changes on screen.
function global:PSConsoleHostReadLine {
    # First statement: any script run resets $?, and PSReadLine shows it in the prompt.
    $__cdgOk = $?
    Microsoft.PowerShell.Core\Set-StrictMode -Off
    $line = try { [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $ExecutionContext, $__cdgOk) }
            catch [Management.Automation.MethodException] {  # PSReadLine < 2.2 has no status argument
                [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $ExecutionContext) }
    $wrap = $line.Trim() -and $line -notmatch '^\s' -and $line -notmatch $global:__cdgSkip -and
            -not ($env:CODIGEST_CAP_SKIP -and $line -match $env:CODIGEST_CAP_SKIP) -and -not $env:CODIGEST_NOCAP
    if ($wrap) {
        $tokens = $null; $errs = $null
        # Single-line form first; the newline form survives a trailing `# comment`.
        # Input that does not parse is returned as typed so the engine reports the error.
        foreach ($w in ". __cdg { $line }", ". __cdg { $line`n}") {
            [void][Management.Automation.Language.Parser]::ParseInput($w, [ref]$tokens, [ref]$errs)
            if (-not $errs) {
                $global:__cdgLine = $line
                return $w
            }
        }
    }
    $line
}

# Lets `cdg term` tell whether this shell is hooked.
$env:CODIGEST_HOOK = '1'
