import re
import shutil
import subprocess
from pathlib import Path

import pytest

HOOK = Path(__file__).parents[1] / "src" / "codigest" / "shell" / "hook.ps1"

# Runs the hook's history restore on a wrapped entry, then on a plain one.
SCRIPT = r"""
$t = Get-Date
[pscustomobject]@{ CommandLine = '. __cdg { git status }'; ExecutionStatus = 'Completed'; StartExecutionTime = $t; EndExecutionTime = $t.AddSeconds(2) } | Add-History
$global:__cdgLine = 'git status'
__cdgRestoreHistory
$h = (Get-History)[-1]
"$($h.CommandLine)|$($h.ExecutionStatus)|$(($h.EndExecutionTime - $h.StartExecutionTime).TotalSeconds)|$($null -eq $global:__cdgLine)"
[pscustomobject]@{ CommandLine = 'ls'; ExecutionStatus = 'Failed'; StartExecutionTime = $t; EndExecutionTime = $t } | Add-History
__cdgRestoreHistory
(Get-History | Select-Object -Last 2).CommandLine -join ','
"""


@pytest.mark.parametrize("shell", ["pwsh", "powershell"])
def test_engine_history_gets_the_line_as_typed(shell):
    if not shutil.which(shell):
        pytest.skip(f"{shell} not installed")
    fn = re.search(
        r"(?s)function global:__cdgRestoreHistory \{.*?\n\}", HOOK.read_text(encoding="utf-8")
    )
    assert fn, "restore function not found in hook.ps1"
    out = subprocess.run(
        [shell, "-NoProfile", "-NonInteractive", "-Command", fn.group(0) + "\n" + SCRIPT],
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    ).stdout.splitlines()
    assert out == ["git status|Completed|2|True", "git status,ls"]
