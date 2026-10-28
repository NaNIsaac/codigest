# Codigest terminal capture hook for PowerShell. Installed by `cdg term install`.
#
# The line you type is run as `. __cdg { <line> }`. The wrapping happens in
# PSConsoleHostReadLine, after PSReadLine has returned the line, so the screen and
# the PSReadLine history keep the line as typed; the engine history entry is put back
# before the next prompt. Dot-sourcing runs it in the caller's scope,
# so `cd`, variable assignments and function definitions behave exactly as typed.
# Output is teed into a buffer and handed to `cdg term ingest` in a background
# process (redaction happens there, before anything touches disk).
#
# Opt out for one command: start the line with a space.
# Opt out entirely: $env:CODIGEST_NOCAP = 1

if ($env:CODIGEST_NOCAP) { return }
$global:__cdgExe = (Get-Command cdg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if (-not $global:__cdgExe) { return }

# Lines kept from the start and from the end of a long output. A dev server left running
# for hours would otherwise fill memory; ingest keeps only 64 KB of it anyway.
$global:__cdgKeep = 2000

# Passes everything through to the screen and keeps the first and last lines. An error
# record that reaches the screen marks the command as failed; one silenced with
# -ErrorAction never gets here.
filter global:__cdgRecord {
    $_
    if ($_ -is [Management.Automation.ErrorRecord]) { $global:__cdgFailed = $true }
    if ($global:__cdgHead.Count -lt $global:__cdgKeep) { $global:__cdgHead.Add($_) }
    else {
        $global:__cdgTail.Enqueue($_)
        if ($global:__cdgTail.Count -gt $global:__cdgKeep) {
            [void]$global:__cdgTail.Dequeue()
            $global:__cdgDropped++
        }
    }
}

# Sets $? to false without printing anything or touching $Error.
function global:__cdgFalse {
    [CmdletBinding()]
    param()
    $ErrorActionPreference = 'Ignore'
    $PSCmdlet.WriteError([Management.Automation.ErrorRecord]::new(
        [Exception]::new('previous command failed'), 'CodigestStatus', 'NotSpecified', $null))
}

function global:__cdg {
    # Advanced function so a failed command can surface as `$? -eq $false` for prompt themes.
    [CmdletBinding()]
    param([scriptblock]$__cdgBlock)
    # A wrapped line run again from inside a captured one (history recall, `hf` from
    # hanfix): the outer call already records it, and a second one would clear its buffers.
    if ($null -ne $global:__cdgHead) {
        . $__cdgBlock
        return
    }
    # Dot-sourced: these locals live in the caller's scope, hence the prefix and cleanup.
    $__cdgCommand = $global:__cdgLine
    # Left as is so the line can read it; a native command stores a new boxed int,
    # so a reference change means one ran, even with the same exit code.
    $__cdgPrevExit = $global:LASTEXITCODE
    $__cdgUnbuf = $env:PYTHONUNBUFFERED
    $env:PYTHONUNBUFFERED = '1'  # piped Python would otherwise block-buffer live output
    $__cdgStart = Get-Date
    $global:__cdgHead = [Collections.Generic.List[object]]::new()
    $global:__cdgTail = [Collections.Generic.Queue[object]]::new()
    $global:__cdgDropped = 0
    $global:__cdgFailed = $false
    # The statements above reset $?; a typed line that reads it must see the previous command's.
    $__cdgFailedBefore = $global:__cdgLastOk -eq $false
    $global:__cdgLastOk = $null
    try {
        if ($__cdgFailedBefore) { __cdgFalse }
        . $__cdgBlock *>&1 | __cdgRecord
    }
    catch {
        # A terminating error: still shown by the host, and the command failed.
        $global:__cdgFailed = $true
        throw
    }
    finally {
        $env:PYTHONUNBUFFERED = $__cdgUnbuf
        $__cdgExit = if (-not [object]::ReferenceEquals($global:LASTEXITCODE, $__cdgPrevExit)) { $global:LASTEXITCODE }
                     elseif ($global:__cdgFailed) { 1 } else { 0 }
        $__cdgText = $global:__cdgHead | Out-String
        if ($global:__cdgDropped) {
            $__cdgText += "`n... [$($global:__cdgDropped) lines not kept by the capture hook] ...`n`n"
        }
        if ($global:__cdgTail.Count) { $__cdgText += $global:__cdgTail | Out-String }
        try {
            $__cdgJson = @{
                command     = $__cdgCommand
                output      = $__cdgText
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
        Remove-Variable __cdgHead, __cdgTail, __cdgDropped, __cdgFailed -Scope Global -ErrorAction Ignore
        if ($__cdgExit) {
            # Sets $? to false without printing anything or touching $Error.
            $__cdgPref = $ErrorActionPreference
            $ErrorActionPreference = 'Ignore'
            $PSCmdlet.WriteError([Management.Automation.ErrorRecord]::new(
                [Exception]::new("exit $__cdgExit"), 'CodigestExit', 'NotSpecified', $null))
            $ErrorActionPreference = $__cdgPref
        }
        Remove-Variable __cdgFailedBefore, __cdgPref, __cdgBlock, __cdgCommand, __cdgPrevExit, __cdgUnbuf, __cdgStart, __cdgText, __cdgExit, __cdgJson, __cdgBytes, __cdgPsi, __cdgProc -ErrorAction Ignore
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
    '|^\s*git\s+(commit(?!.*\s-\w*[mF])|rebase\s+-i|add\s+-[pi]|mergetool)' +
    # Interactive prompts: a question that does not end its line stays invisible in the
    # pipe until the next newline, so the command looks stuck.
    '|^\s*(docker|podman|kubectl)\s+(run|exec|attach|start)\b.*\s(-\w*[it]\w*|--interactive|--tty)\b' +
    '|^\s*(npm|pnpm|yarn|bun)\s+(init|create)\b|^\s*npx\s+create-' +
    '|^\s*pip3?\s+uninstall\b(?!.*\s(-y|--yes)\b)' +
    '|^\s*(gh\s+auth\s+login|az\s+login|gcloud\s+(auth\s+login|init)|aws\s+configure|ssh-keygen|ssh-copy-id|sudo|runas)\b' +
    '|^\s*(mysql|psql|sqlite3|redis-cli|mongosh|duckdb|julia|irb|ghci|ollama\s+run)(\.exe)?\b' 

# The engine history (`history`, `r`) records the line handed to the engine, the wrapped
# one. Before the next prompt the last entry is put back as typed, with its status and
# times; its new entry takes the next id, so ids skip one after each captured line.
function global:__cdgRestoreHistory {
    $line = $global:__cdgLine
    $global:__cdgLine = $null
    $last = Get-History -Count 1
    if (-not $line -or -not $last -or -not $last.CommandLine.StartsWith('. __cdg { ')) { return }
    Clear-History -Id $last.Id
    [pscustomobject]@{
        CommandLine        = $line
        ExecutionStatus    = $last.ExecutionStatus
        StartExecutionTime = $last.StartExecutionTime
        EndExecutionTime   = $last.EndExecutionTime
    } | Add-History
}

# Whether a line is captured. A program that runs other command lines (hanfix's hf) lists
# its names in $global:CODIGEST_RUNNERS: its own line is not captured, so it keeps the
# terminal, and it hands each line it runs to __cdg_run, which decides for that line.
function global:__cdgWants([string]$line) {
    $line.Trim() -and $line -notmatch '^\s' -and $line -notmatch $global:__cdgSkip -and
        -not ($env:CODIGEST_CAP_SKIP -and $line -match $env:CODIGEST_CAP_SKIP) -and -not $env:CODIGEST_NOCAP -and
        ($line.Trim() -split '\s+', 2)[0] -notin @($global:CODIGEST_RUNNERS)
}

# Runs a command line for a runner, captured as if it had been typed. Dot-source it
# (`. __cdg_run $line`) so the line runs in the runner's scope, as Invoke-Expression would.
function global:__cdg_run([string]$__cdgRunLine) {
    $__cdgRunBlock = [scriptblock]::Create($__cdgRunLine)
    if ($null -eq $global:__cdgHead -and (__cdgWants $__cdgRunLine)) {
        $global:__cdgLine = $__cdgRunLine
        . __cdg $__cdgRunBlock
    }
    else { . $__cdgRunBlock }
}

# A program that turns a typed line into another one before it runs (hanfix's hf: the previous
# command, fixed) lists a function in $global:CODIGEST_REWRITERS. It gets the line and returns $null
# (not its line), the new line, or @{ Line = <new>; Confirm = $true } to ask first. The new line then
# goes on as if typed: histories, capture rules, $?. A rewriter that fails, returns anything else or
# answers after more than 200 ms leaves the line as typed (not a timeout: it is waited for), and it
# does not touch history itself.
function global:__cdgRewrite([string]$line) {
    # Also turns a rewriter's non-terminating error into one the catch sees.
    $ErrorActionPreference = 'Stop'
    foreach ($name in @($global:CODIGEST_REWRITERS).Where({ $_ })) {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        try { $new = & $name $line } catch { return $line }
        if ($clock.ElapsedMilliseconds -gt 200) { return $line }
        if ($null -eq $new) { continue }
        $ask = $new -is [Collections.IDictionary] -and $new.Confirm
        if ($new -is [Collections.IDictionary]) { $new = $new.Line }
        if ($new -isnot [string] -or -not $new.Trim()) { return $line }
        $tokens = $null; $errs = $null
        [void][Management.Automation.Language.Parser]::ParseInput($new, [ref]$tokens, [ref]$errs)
        if ($errs) { return $line }
        # A line typed with a leading space stays out of the capture.
        if ($line -match '^\s' -and $new -notmatch '^\s') { $new = " $new" }
        if ($ask -and -not (__cdgAsk $new)) { return '' }
        return $new
    }
    $line
}

# Enter runs the line; any other key, Ctrl+C included, drops it.
function global:__cdgAsk([string]$line) {
    Write-Host "$line  [Enter / Ctrl+C] " -ForegroundColor Green -NoNewline
    $ctrlC = [Console]::TreatControlCAsInput
    try {
        [Console]::TreatControlCAsInput = $true
        $key = [Console]::ReadKey($true)
    }
    finally {
        [Console]::TreatControlCAsInput = $ctrlC
        Write-Host ''
    }
    $key.Key -eq 'Enter' -and -not $key.Modifiers
}

# PSReadLine adds a line to its history before it returns it, while a rewriter may still replace it.
# So while the read-line below reads, the handler holds the line back, and the read-line adds the line
# that runs. The handler set before (PSReadLine's own leaves out lines that look like secrets) still
# decides for that line. Loading the hook again keeps the chain as it is.
if (-not $global:__cdgHistory) {
    $global:__cdgHistory = $true
    $global:__cdgHistoryPrev = (Get-PSReadLineOption).AddToHistoryHandler
    Set-PSReadLineOption -AddToHistoryHandler {
        param([string]$line)
        if ($global:__cdgDefer) {
            $global:__cdgDeferred = $true
            return $false
        }
        $prev = $global:__cdgHistoryPrev
        # PSReadLine's own is a Func<string, object>, one set by a profile a script block. An error
        # here would bring PSReadLine down, so a failing one keeps the line, as no handler would.
        try {
            if ($prev -is [Delegate]) { $prev.Invoke($line) } elseif ($prev) { & $prev $line } else { $true }
        }
        catch { $true }
    }
}

function global:__cdgReadLine([bool]$ok) {
    try { [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $ExecutionContext, $ok) }
    catch [Management.Automation.MethodException] {  # PSReadLine < 2.2 has no status argument
        [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $ExecutionContext) }
}

# Another PSConsoleHostReadLine (atuin, a terminal's shell integration) is called for the line
# and gets it as typed; codigest wraps only what it returns. PSReadLine's own is not one: it
# would lose the last status. Loading the hook again keeps the one found the first time.
$__cdgTop = Get-Item function:PSConsoleHostReadLine -ErrorAction Ignore
if ($__cdgTop -and $__cdgTop.Module.Name -ne 'PSReadLine' -and $__cdgTop.Definition -notmatch 'codigest-readline') {
    $global:__cdgPrev = $__cdgTop.ScriptBlock
}
Remove-Variable __cdgTop

# Replaces PSConsoleHostReadLine, which the host calls for every prompt. PSReadLine draws and
# stores the line as typed; only the string returned to the engine is wrapped, so nothing
# changes on screen.
function global:PSConsoleHostReadLine {
    # codigest-readline (marks this function as codigest's)
    # First statement: any script run resets $?, and PSReadLine shows it in the prompt.
    $__cdgOk = $?
    Microsoft.PowerShell.Core\Set-StrictMode -Off
    # Called back by the function this one calls: read the line and nothing more.
    if ($global:__cdgReading) { return __cdgReadLine $global:__cdgOk }
    __cdgRestoreHistory
    # A program that loaded later (VS Code's shell integration) wrapped this function and calls
    # it for the line. Put this one back on top, around that one, for the next line; this line
    # goes to it as typed and is not captured.
    if ((Get-Item function:PSConsoleHostReadLine).Definition -notmatch 'codigest-readline') {
        $global:__cdgPrev = (Get-Item function:PSConsoleHostReadLine).ScriptBlock
        Set-Item function:global:PSConsoleHostReadLine $global:__cdgSelf
        return __cdgReadLine $__cdgOk
    }
    # Not in a remote session, a nested prompt or the debugger: there the line is for
    # another runspace or is a debugger command (c, s, v), and must reach it as typed.
    $local = -not $host.Runspace.RunspaceIsRemote -and -not $NestedPromptLevel -and -not $PSDebugContext
    $rewrite = $local -and @($global:CODIGEST_REWRITERS).Where({ $_ }).Count
    $global:__cdgDeferred = $false
    $global:__cdgDefer = $rewrite
    try {
        $line = if ($global:__cdgPrev) {
            $global:__cdgOk = $__cdgOk
            $global:__cdgReading = $true
            try { & $global:__cdgPrev } finally { $global:__cdgReading = $false }
        }
        else { __cdgReadLine $__cdgOk }
    }
    finally { $global:__cdgDefer = $false }
    if ($rewrite) {
        $line = __cdgRewrite $line
        if ($global:__cdgDeferred -and $line) {
            try { [Microsoft.PowerShell.PSConsoleReadLine]::AddToHistory($line) } catch { }
        }
    }
    $wrap = $local -and (__cdgWants $line)
    if ($wrap) {
        $tokens = $null; $errs = $null
        # Single-line form first; the newline form survives a trailing `# comment`.
        # Input that does not parse is returned as typed so the engine reports the error.
        foreach ($w in ". __cdg { $line }", ". __cdg { $line`n}") {
            [void][Management.Automation.Language.Parser]::ParseInput($w, [ref]$tokens, [ref]$errs)
            if (-not $errs) {
                $global:__cdgLine = $line
                $global:__cdgLastOk = $__cdgOk
                return $w
            }
        }
    }
    $line
}
$global:__cdgSelf = (Get-Item function:PSConsoleHostReadLine).ScriptBlock

# A program that loads later and never calls the function it replaced (atuin reads the line
# itself) would stop the capture for good. While PSReadLine waits for a line, put this one back
# on top around that one; from the next prompt on, codigest is called again. -SupportEvent
# keeps the subscription out of Get-Job, and the flag keeps a reload from adding a second one.
if (-not $global:__cdgIdle) {
    $global:__cdgIdle = $true
    Register-EngineEvent -SourceIdentifier PowerShell.OnIdle -SupportEvent -Action {
        $top = Get-Item function:PSConsoleHostReadLine -ErrorAction Ignore
        if ($top -and $top.Module.Name -ne 'PSReadLine' -and $top.Definition -notmatch 'codigest-readline') {
            __cdgRestoreHistory
            $global:__cdgPrev = $top.ScriptBlock
            Set-Item function:global:PSConsoleHostReadLine $global:__cdgSelf
        }
    }
}

# Lets `cdg term` tell whether this shell is hooked.
$env:CODIGEST_HOOK = '1'
