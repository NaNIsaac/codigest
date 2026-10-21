# Optional PowerShell Get-Help integration (cdg shell-help install).
# Does not modify PowerShell's built-in help function or enable terminal capture.
if (-not (Get-Command codigest -CommandType Application -ErrorAction SilentlyContinue)) {
    return
}

function global:codigest {
    <#
    .SYNOPSIS
        Share selected code and terminal diagnostics with an AI.
    .DESCRIPTION
        Codigest prepares code and command output for pasting into ChatGPT,
        Claude or another AI. Scan files, inspect changes or copy recently
        captured terminal output. Run codigest --help for all commands.
    .EXAMPLE
        codigest --help
        Show all commands and options.
    .EXAMPLE
        codigest scan src/app.py
        Copy the source file as context.
    .EXAMPLE
        codigest term --context
        Copy the last command and output with machine and shell details.
    .EXAMPLE
        codigest cap -- pytest -q
        Run and capture a command with its output and exit code.
    .LINK
        https://github.com/NaNIsaac/codigest
    #>
    & (Get-Command codigest -CommandType Application -ErrorAction Stop |
        Select-Object -First 1).Source @args
}
