"""
Git Operations (read-only, against the user's real repository).
"""
lazy import subprocess
from pathlib import Path


def is_git_repo(root_path: Path) -> bool:
    return (root_path / ".git").exists()


def _git(root_path: Path, *args: str) -> str | None:
    result = subprocess.run(
        ["git", *args], cwd=root_path, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False
    )
    return result.stdout if result.returncode == 0 else None


def working_changes(root_path: Path) -> tuple[str, list[Path]] | None:
    """
    `git diff HEAD` (staged + unstaged) and the untracked files, or None outside a
    repository with commits. Untracked files are returned as paths, not inlined
    into the diff, so callers can budget them like any other file.
    """
    if not is_git_repo(root_path) or (diff := _git(root_path, "diff", "HEAD")) is None:
        return None
    untracked = _git(root_path, "ls-files", "--others", "--exclude-standard") or ""
    return diff, [root_path / line for line in untracked.splitlines()]
