import re
import shutil
import subprocess
from pathlib import Path

import pytest

HOOK = Path(__file__).parents[1] / "src" / "codigest" / "shell" / "hook.ps1"


@pytest.mark.parametrize("shell", ["pwsh", "powershell"])
def test_wants_preserves_pagers(shell):
    if not shutil.which(shell):
        pytest.skip(f"{shell} not installed")
    hook = HOOK.read_text(encoding="utf-8")
    skip = re.search(r"(?s)\$global:__cdgSkip = .*?(?=\n\n)", hook)
    wants = re.search(r"(?s)function global:__cdgWants\(.*?\n\}", hook)
    assert skip and wants, "capture rules not found in hook.ps1"
    script = r"""$env:CODIGEST_CAP_SKIP = $null
$env:CODIGEST_NOCAP = $null
$global:CODIGEST_RUNNERS = @()
@('bat x.py', 'batcat x.py', 'delta a b', 'glow README.md', 'man git',
  'git diff', 'git log --oneline', 'git show', 'git blame x.py',
  'git -p status', 'git --paginate status', 'git.exe diff',
  'git status', 'python x.py', 'git --no-pager status', 'git logger') |
    ForEach-Object { __cdgWants $_ }
"""
    out = subprocess.run(
        [
            shell, "-NoProfile", "-NonInteractive", "-Command",
            skip.group(0) + "\n" + wants.group(0) + "\n" + script,
        ],
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    ).stdout.splitlines()
    assert out == ["False"] * 12 + ["True"] * 4
