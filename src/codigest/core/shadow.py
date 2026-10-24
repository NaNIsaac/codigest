"""
Context Anchor Engine.
Modified to hide internal git repository from VS Code by renaming .git -> .shadow_git
"""

lazy import shutil
lazy from .common import hide_from_git, remove_tree
lazy import subprocess
import re
import time
from pathlib import Path, PurePosixPath
from loguru import logger

# Directories that belong to git, the anchor or Codigest and never to the project.
_INTERNAL_DIRS = frozenset({".git", ".codigest", ".shadow_git"})
_GIT_ESCAPES = {"a": 7, "b": 8, "f": 12, "n": 10, "r": 13, "t": 9, "v": 11, '"': 34, "\\": 92}
_OCTAL = "01234567"


def _quoted_end(text: str) -> int:
    """Index of the quote that closes the quoted token `text` starts with, or -1."""
    i = 1
    while i < len(text):
        if text[i] == "\\":
            i += 2
        elif text[i] == '"':
            return i
        else:
            i += 1
    return -1


def _unquote(token: str) -> str:
    """Undoes git's C-style quoting of a path: `"a/\\303\\245"` -> `a/å`."""
    body, out, i = token[1:-1], bytearray(), 0
    while i < len(body):
        ch = body[i]
        if ch != "\\" or i + 1 == len(body):
            out += ch.encode("utf-8")
            i += 1
        elif body[i + 1] in _OCTAL:
            j = i + 1
            while j < len(body) and j < i + 4 and body[j] in _OCTAL:
                j += 1
            out.append(int(body[i + 1 : j], 8) & 0xFF)
            i = j
        else:
            escaped = body[i + 1]
            if (code := _GIT_ESCAPES.get(escaped)) is not None:
                out.append(code)
            else:
                out += escaped.encode("utf-8")
            i += 2
    return out.decode("utf-8", errors="replace")


def diff_header_paths(line: str) -> tuple[str, str] | None:
    """
    The old and new path of a `diff --git` line, without git's `a/` and `b/` prefixes; None
    if the line is not such a header.

    A name with a space is written bare, so `a/my dir/c.md b/my dir/c.md` is split where both
    halves read the same. A name with a quote, a tab or non-ASCII characters is written in
    quotes with C-style escapes. If the two names differ and contain " b/" the split is a guess.
    """
    rest = line.removeprefix("diff --git ")
    if rest == line or not rest:
        return None

    if rest.startswith('"'):
        end = _quoted_end(rest)
        if end == -1 or rest[end + 1 : end + 2] != " ":
            return None
        old, raw = _unquote(rest[: end + 1]), rest[end + 2 :]
        new = _unquote(raw) if raw.startswith('"') else raw
    elif (cut := rest.find(' "b/')) != -1:
        old, new = rest[:cut], _unquote(rest[cut + 1 :])
    else:
        old = new = None
        half, odd = divmod(len(rest) - 5, 2)
        if half > 0 and not odd:
            left, right = rest[: 2 + half], rest[3 + half :]
            if (
                rest[2 + half] == " "
                and left[2:] == right[2:]
                and left.startswith("a/")
                and right.startswith("b/")
            ):
                old, new = left, right
        if old is None:
            for found in re.finditer(" b/", rest):
                if rest.startswith("a/"):
                    old, new = rest[: found.start()], rest[found.start() + 1 :]
                    break
            else:
                return None

    if not (old.startswith("a/") and new.startswith("b/")):
        return None
    return old[2:], new[2:]


def _is_internal(header: str) -> bool:
    """
    Whether a `diff --git` section is about git's or the anchor's own files. Whole path
    components, so `.gitignore`, `.github/` and `.gitattributes` are ordinary project files.
    """
    paths = diff_header_paths(header)
    return paths is not None and any(_INTERNAL_DIRS & set(PurePosixPath(p).parts) for p in paths)


class ContextAnchor:
    def __init__(self, root_path: Path):
        self.root = root_path.resolve()
        self.anchor_dir = (self.root / ".codigest" / "anchor").resolve()
        self.git_dir = (self.anchor_dir / ".shadow_git").resolve()

    def has_history(self) -> bool:
        """Checks if a valid anchor (git repo with commits) exists."""
        return self.git_dir.exists() and (self.git_dir / "HEAD").exists()

    def _run_git(self, args: list[str], cwd: Path | None = None, check: bool = True) -> str:
        self.anchor_dir.mkdir(parents=True, exist_ok=True)
        cmd = [
            "git",
            "--git-dir",
            str(self.git_dir),
            "--work-tree",
            str(self.anchor_dir),
            *args,
        ]

        target_dir = cwd or self.anchor_dir

        result = subprocess.run(
            cmd, cwd=target_dir, capture_output=True, text=True, encoding="utf-8", errors="replace"
        )

        if check and result.returncode != 0:
            logger.debug(f"Shadow Git Warning ({args[0]}): {result.stderr.strip()}")

        return (result.stdout or "").strip()

    def has_changes(self, current_files: list[Path]) -> bool:
        """Fast check to determine if any file was modified/added/deleted since last anchor baseline."""
        if not self.has_history() or not self.anchor_dir.exists():
            return True

        current_rel_paths = set()
        for src in current_files:
            if ".git" in src.parts or ".codigest" in src.parts:
                continue
            try:
                rel = src.relative_to(self.root)
                current_rel_paths.add(rel)
                dest = self.anchor_dir / rel
                if not dest.exists():
                    return True
                src_stat = src.stat()
                dest_stat = dest.stat()
                # Use size + 1ms mtime tolerance for filesystem precision differences
                if (
                    src_stat.st_size != dest_stat.st_size
                    or (src_stat.st_mtime_ns - dest_stat.st_mtime_ns) > 1_000_000
                ):
                    return True
            except Exception:
                return True

        for anchor_file in self.anchor_dir.rglob("*"):
            if (
                anchor_file.is_file()
                and ".shadow_git" not in anchor_file.parts
                and anchor_file.name != ".gitignore"
            ):
                try:
                    rel = anchor_file.relative_to(self.anchor_dir)
                    if rel not in current_rel_paths:
                        return True
                except Exception:
                    continue

        return False

    def update(self, source_files: list[Path]) -> None:
        self.anchor_dir.mkdir(parents=True, exist_ok=True)
        hide_from_git(self.root / ".codigest")
        (self.anchor_dir / ".gitignore").write_text(".shadow_git\n", encoding="utf-8")
        if not self.has_history():
            if self.git_dir.exists():
                remove_tree(self.git_dir)
            subprocess.run(
                ["git", "init", "--bare", str(self.git_dir)], capture_output=True, check=False
            )
            self._run_git(["config", "core.bare", "false"])
            self._run_git(["config", "user.email", "codigest@ai"])
            self._run_git(["config", "user.name", "Context Manager"])
            self._run_git(["config", "core.autocrlf", "false"])
            self._run_git(["config", "gc.auto", "0"])

        # Quick check: if nothing changed since last anchor, skip work
        if self.has_history() and not self.has_changes(source_files):
            return

        source_rel_paths = set()

        for src in source_files:
            if ".git" in src.parts or ".codigest" in src.parts:
                continue
            try:
                rel = src.relative_to(self.root)
                dest = self.anchor_dir / rel

                if not dest.parent.exists():
                    dest.parent.mkdir(parents=True, exist_ok=True)

                should_copy = True
                if dest.exists():
                    src_stat = src.stat()
                    dest_stat = dest.stat()

                    if (
                        src_stat.st_size == dest_stat.st_size
                        and dest_stat.st_mtime_ns >= src_stat.st_mtime_ns
                    ):
                        should_copy = False

                source_rel_paths.add(rel)
                if should_copy:
                    shutil.copy2(src, dest)
            except Exception:
                continue

        for anchor_file in self.anchor_dir.rglob("*"):
            if anchor_file.is_file():
                if ".shadow_git" in anchor_file.parts:
                    continue

                try:
                    rel = anchor_file.relative_to(self.anchor_dir)
                    if rel not in source_rel_paths:
                        anchor_file.unlink()

                except Exception:
                    continue

        self._run_git(["add", "."])
        if not self._run_git(["rev-parse", "--verify", "HEAD"], check=False):
            self._run_git(["commit", "-m", f"Snapshot: {int(time.time())}"])
            logger.info("Initial context anchor created.")
        else:
            # --quiet reports changes only through the exit code, never on stdout.
            dirty = (
                subprocess.run(
                    [
                        "git",
                        "--git-dir",
                        str(self.git_dir),
                        "--work-tree",
                        str(self.anchor_dir),
                        "diff-index",
                        "--quiet",
                        "--cached",
                        "HEAD",
                        "--",
                    ],
                    cwd=self.anchor_dir,
                    capture_output=True,
                ).returncode
                != 0
            )
            if dirty:
                self._run_git(["commit", "-m", f"Snapshot: {int(time.time())}"])
                logger.info("Context anchor updated.")

    def get_changes(self, current_files: list[Path]) -> str:
        if not self.git_dir.exists():
            return ""

        if not self.has_changes(current_files):
            return ""

        temp_current = self.root / ".codigest" / "temp_diff_current"
        temp_baseline = self.root / ".codigest" / "temp_diff_baseline"

        for d in [temp_current, temp_baseline]:
            if d.exists():
                remove_tree(d)
            d.mkdir(parents=True)

        try:
            current_rel_paths = set()
            for src in current_files:
                if ".git" in src.parts or ".codigest" in src.parts:
                    continue
                try:
                    rel = src.relative_to(self.root)
                    current_rel_paths.add(rel)
                    dest = temp_current / rel
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(src, dest)
                except Exception:
                    continue

            subprocess.run(
                [
                    "git",
                    "--git-dir",
                    str(self.git_dir),
                    "--work-tree",
                    str(temp_baseline),
                    "checkout",
                    "HEAD",
                    "--",
                    ".",
                ],
                capture_output=True,
                check=False,
            )

            self._prune_ignored_files(temp_baseline, current_rel_paths)

            result = subprocess.run(
                ["git", "diff", "--no-index", "temp_diff_baseline", "temp_diff_current"],
                cwd=self.root / ".codigest",
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
            )

            diff_text = result.stdout or ""
            diff_text = diff_text.replace("temp_diff_baseline/", "").replace(
                "temp_diff_current/", ""
            )

            clean_lines = []
            skip = False
            for line in diff_text.splitlines():
                if line.startswith("diff --git"):
                    skip = _is_internal(line)
                if not skip:
                    clean_lines.append(line)
            return "\n".join(clean_lines)

        finally:
            for d in [temp_current, temp_baseline]:
                if d.exists():
                    remove_tree(d)

    def _prune_ignored_files(self, baseline_dir: Path, valid_rel_paths: set[Path]) -> None:
        """
        Drops baseline files that are still in the project but were not scanned (newly ignored,
        or outside the filters), so they are not reported as deleted. A file that is gone from
        disk stays and shows as deleted. Judged by the path inside the baseline: the absolute
        one always runs through `.codigest`, which once made this skip every file.
        """
        for file_path in baseline_dir.rglob("*"):
            if not file_path.is_file():
                continue
            try:
                rel_path = file_path.relative_to(baseline_dir)
                if _INTERNAL_DIRS & set(rel_path.parts) or rel_path in valid_rel_paths:
                    continue
                if (self.root / rel_path).exists():
                    file_path.unlink()
            except Exception:
                continue

    def get_last_update_time(self) -> str:
        if not self.git_dir.exists():
            return "Never"
        try:
            return self._run_git(["log", "-1", "--format=%cr"])
        except (subprocess.SubprocessError, FileNotFoundError) as e:
            logger.warning(f"Git history lookup failed: {e}")
            return "Unknown"

    def read_anchor_file(self, rel_path: Path) -> str:
        if not self.git_dir.exists():
            return ""

        git_path = rel_path.as_posix()

        result = subprocess.run(
            [
                "git",
                "--git-dir",
                str(self.git_dir),
                "--work-tree",
                str(self.anchor_dir),
                "show",
                f"HEAD:{git_path}",
            ],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )

        if result.returncode != 0:
            return ""

        return result.stdout

    def get_changed_files(self, current_files: list[Path]) -> list[Path]:
        raw_diff = self.get_changes(current_files)
        paths: set[Path] = set()
        for line in raw_diff.splitlines():
            if (header := diff_header_paths(line)) and not _is_internal(line):
                paths.add(self.root / header[1])
        return sorted(paths)
