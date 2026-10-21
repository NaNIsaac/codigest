"""
Native Shell History Readers (fallback path).

When a command was run without `cdg cap`, its output is gone -- no shell stores
it. The command text, however, is recoverable from the shell's own history file.
This module provides that degraded fallback so `cdg term` is still useful in a
terminal that was never wrapped.
"""

import os
import re
import sys
from collections.abc import Callable
from pathlib import Path

from .termstore import Entry

_SELF_RE = re.compile(r"^\s*(cdg|codigest)(\.exe)?\s+term\b")
_ZSH_RE = re.compile(r"^:\s*\d+:\d+;(.*)$")
_FISH_RE = re.compile(r"^-\s+cmd:\s*(.*)$")

type HistoryParser = Callable[[Path], list[str]]


def _psreadline_path() -> Path | None:
    if not (appdata := os.environ.get("APPDATA")):
        return None
    return (
        Path(appdata)
        / "Microsoft"
        / "Windows"
        / "PowerShell"
        / "PSReadLine"
        / "ConsoleHost_history.txt"
    )


def _read_lines(path: Path) -> list[str]:
    with open(path, encoding="utf-8", errors="replace") as f:
        return [ln.rstrip("\n") for ln in f]


def _parse_plain(path: Path) -> list[str]:
    """PSReadLine / bash: one command per line (bash may carry `#<ts>` lines)."""
    return [line for line in _read_lines(path) if line.strip() and not line.startswith("#")]


def _parse_zsh(path: Path) -> list[str]:
    """zsh extended history: `: <start>:<elapsed>;<command>`."""
    return [
        match.group(1) if (match := _ZSH_RE.match(line)) else line
        for line in _read_lines(path)
        if line.strip()
    ]


def _parse_fish(path: Path) -> list[str]:
    """fish history: YAML-ish `- cmd: <command>` records."""
    return [match.group(1) for line in _read_lines(path) if (match := _FISH_RE.match(line))]


def _candidates() -> list[tuple[str, Path | None, HistoryParser]]:
    home = Path.home()
    return [
        ("powershell", _psreadline_path(), _parse_plain),
        ("zsh", home / ".zsh_history", _parse_zsh),
        ("bash", home / ".bash_history", _parse_plain),
        ("fish", home / ".local" / "share" / "fish" / "fish_history", _parse_fish),
    ]


def detect_shell() -> str:
    """Best-effort name of the shell that most likely launched this process."""
    if shell := os.environ.get("SHELL"):
        return Path(shell).stem
    if os.environ.get("PSModulePath"):
        return "powershell"
    if sys.platform == "win32":
        return "cmd"
    return "unknown"


def read_recent(count: int) -> tuple[list[Entry], str]:
    """
    Reads the most recent `count` commands from the newest available shell
    history file. Returns (entries oldest-first, shell name). Entries carry no
    output and are flagged `captured=False`.
    """
    best: tuple[float, str, list[str]] | None = None

    for name, path, parser in _candidates():
        if not path or not path.exists():
            continue
        try:
            commands = parser(path)
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if not commands:
            continue
        if best is None or mtime > best[0]:
            best = (mtime, name, commands)

    if best is None:
        return [], detect_shell()

    _, shell, commands = best
    # The history file already holds the `cdg term` call that is reading it.
    while commands and _SELF_RE.match(commands[-1]):
        commands.pop()
    recent = commands[-count:] if count > 0 else commands
    entries = [
        Entry(
            command=cmd,
            shell=shell,
            captured=False,
            note="output not captured -- command was not run under `cdg cap`",
        )
        for cmd in recent
    ]
    return entries, shell
