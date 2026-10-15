"""
Global Terminal Capture Store.

Unlike the rest of Codigest, this store is machine-global rather than
project-scoped: terminal captures are meant to work in any directory, with or
without a repository. It deliberately avoids `ProjectContext` and the project
`.codigest/` directory.

Layout:
    <data_dir>/term/history.jsonl   -- newline-delimited entries, oldest first

Storage is a ring buffer: at most MAX_ENTRIES entries, each output capped at
MAX_OUTPUT_BYTES (head + tail preserved, middle elided).
"""
lazy import html
lazy import json
import os
import re
import sys
from dataclasses import asdict, dataclass, fields
from datetime import datetime
from pathlib import Path
from typing import Any, Self

from . import tags
from .redact import redact, is_denied_command

MAX_ENTRIES = 200
_TRIM_SLACK = 50
MAX_OUTPUT_BYTES = 64 * 1024
_HEAD_BYTES = MAX_OUTPUT_BYTES // 2
_TAIL_BYTES = MAX_OUTPUT_BYTES - _HEAD_BYTES

# CSI / OSC / single-char escapes emitted by colored CLI output.
_ANSI_RE = re.compile(
    r"\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)"  # OSC ... BEL / ST
    r"|\x1B[@-Z\\-_]"  # Fe escapes
    r"|\x1B\[[0-?]*[ -/]*[@-~]"  # CSI
)


# --------------------------------------------------------------------------
# Paths
# --------------------------------------------------------------------------

def get_data_dir() -> Path:
    """
    Resolves the machine-global Codigest data directory.
    Honors CODIGEST_HOME, then platform conventions.
    """
    if override := os.environ.get("CODIGEST_HOME"):
        return Path(override).expanduser()

    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA") or (Path.home() / "AppData" / "Local")
        return Path(base) / "codigest"

    if base := os.environ.get("XDG_DATA_HOME"):
        return Path(base) / "codigest"
    return Path.home() / ".local" / "share" / "codigest"


def get_term_dir() -> Path:
    return get_data_dir() / "term"


def get_history_path() -> Path:
    return get_term_dir() / "history.jsonl"


# --------------------------------------------------------------------------
# Entry model
# --------------------------------------------------------------------------

@dataclass(slots=True)
class Entry:
    command: str
    output: str = ""
    exit_code: int | None = None
    cwd: str = ""
    shell: str = ""
    started_at: str = ""
    duration_ms: int = 0
    truncated: bool = False
    captured: bool = True  # False => reconstructed from shell history (no output)
    note: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> Self:
        known = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in data.items() if k in known})


# --------------------------------------------------------------------------
# Output normalization
# --------------------------------------------------------------------------

def strip_ansi(text: str) -> str:
    return _ANSI_RE.sub("", text)


def collapse_carriage_returns(text: str) -> str:
    """
    Collapses progress-bar redraws: within a line, only the final `\\r` segment
    survives. Without this a single pytest/pip run can store megabytes of
    overwritten frames. CRLF line endings (Windows) are not redraws.
    """
    text = text.replace("\r\n", "\n")
    if "\r" not in text:
        return text
    return "\n".join(line.rpartition("\r")[2] for line in text.split("\n"))


def normalize_output(raw: str) -> str:
    return collapse_carriage_returns(strip_ansi(raw)).rstrip()


def _truncate(text: str) -> tuple[str, bool]:
    encoded = text.encode("utf-8", errors="replace")
    if len(encoded) <= MAX_OUTPUT_BYTES:
        return text, False

    head = encoded[:_HEAD_BYTES].decode("utf-8", errors="ignore")
    tail = encoded[-_TAIL_BYTES:].decode("utf-8", errors="ignore")
    elided = len(encoded) - _HEAD_BYTES - _TAIL_BYTES
    marker = f"\n\n... [{elided} bytes elided by codigest] ...\n\n"
    return head + marker + tail, True


# --------------------------------------------------------------------------
# Read / write
# --------------------------------------------------------------------------

def _inside(path: str, root: Path) -> bool:
    path, base = (os.path.normcase(os.path.abspath(p)) for p in (path, root))
    return path == base or path.startswith(base.rstrip(os.sep) + os.sep)


def select(entries: list[Entry], *, failed: bool = False, under: Path | None = None) -> list[Entry]:
    """Keeps entries that exited non-zero (`failed`) and/or ran inside `under` or below it."""
    if failed:
        entries = [e for e in entries if e.exit_code not in (None, 0)]
    if under is not None:
        entries = [e for e in entries if e.cwd and _inside(e.cwd, under)]
    return entries


def load_entries() -> list[Entry]:
    """Loads all stored entries, oldest first. Corrupt lines are skipped."""
    path = get_history_path()
    if not path.exists():
        return []

    entries: list[Entry] = []
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for raw in f:
            if not (line := raw.strip()):
                continue
            try:
                entries.append(Entry.from_dict(json.loads(line)))
            except (json.JSONDecodeError, TypeError):
                continue
    # Shell-hook ingests run in background processes and may land out of order.
    entries.sort(key=lambda e: e.started_at)
    return entries


def append_entry(entry: Entry, allow_secrets: bool = False) -> None:
    """
    Normalizes, redacts, truncates and appends an entry, then trims the ring
    buffer. Denied commands are stored without their output.
    """
    entry.output = normalize_output(entry.output)

    if not allow_secrets:
        if is_denied_command(entry.command):
            entry.output = ""
            entry.note = "output withheld (command may expose credentials)"
        else:
            entry.output = redact(entry.output)
            entry.command = redact(entry.command)

    entry.output, entry.truncated = _truncate(entry.output)

    term_dir = get_term_dir()
    term_dir.mkdir(parents=True, exist_ok=True)
    path = get_history_path()

    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(asdict(entry), ensure_ascii=False) + "\n")

    _trim(path)
    _restrict_permissions(term_dir, path)


def _trim(path: Path) -> None:
    """Rewrites the log keeping only the most recent MAX_ENTRIES lines."""
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    # No file lock: Concurrent hook ingests can drop an entry appended
    # between read and replace; the slack makes rewrites (and that window) rare.
    if len(lines) <= MAX_ENTRIES + _TRIM_SLACK:
        return
    tmp = path.with_suffix(".jsonl.tmp")
    with open(tmp, "w", encoding="utf-8") as f:
        f.writelines(lines[-MAX_ENTRIES:])
    tmp.replace(path)


def _restrict_permissions(term_dir: Path, path: Path) -> None:
    """Best-effort owner-only permissions on POSIX; a no-op on Windows."""
    if sys.platform == "win32":
        return
    try:
        term_dir.chmod(0o700)
        path.chmod(0o600)
    except OSError:
        pass


def clear() -> int:
    """Deletes the capture log. Returns the number of entries removed."""
    path = get_history_path()
    if not path.exists():
        return 0
    count = len(load_entries())
    path.unlink()
    return count


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------

def _fmt_duration(ms: int) -> str:
    if ms < 1000:
        return f"{ms}ms"
    return f"{ms / 1000:.1f}s"


def render(entries: list[Entry]) -> str:
    """
    Renders entries as an XML block. `index` is 1-based counting backwards:
    index="1" is the most recent command, index="3" is three commands ago.
    """
    if not entries:
        return "<terminal_history />"

    blocks = []
    total = len(entries)
    for offset, entry in enumerate(entries):
        index = total - offset
        attrs = [
            f'index="{index}"',
            f'command="{html.escape(entry.command)}"',
        ]
        if entry.exit_code is not None:
            attrs.append(f'exit_code="{entry.exit_code}"')
        if entry.cwd:
            attrs.append(f'cwd="{html.escape(entry.cwd)}"')
        if entry.duration_ms:
            attrs.append(f'duration="{_fmt_duration(entry.duration_ms)}"')
        if entry.started_at:
            attrs.append(f'at="{entry.started_at}"')
        if entry.truncated:
            attrs.append('truncated="true"')
        if not entry.captured:
            attrs.append('output_available="false"')

        header = f"<command {' '.join(attrs)}>"
        body = []
        if entry.note:
            body.append(f"<note>{tags.escape_xml_value(entry.note)}</note>")
        if entry.captured or entry.output:
            body.append(
                "<output>\n"
                + tags.escape_xml_value(entry.output)
                + "\n</output>"
            )
        blocks.append(header + "\n" + "\n".join(body) + "\n</command>")

    return "<terminal_history>\n" + "\n".join(blocks) + "\n</terminal_history>"


def now_iso() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")
