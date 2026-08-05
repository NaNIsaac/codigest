"""
Context Anchor Engine.
Modified to hide internal git repository from VS Code by renaming .git -> .shadow_git
"""
import shutil
import subprocess
import time
from pathlib import Path
from loguru import logger

class ContextAnchor:
    def __init__(self, root_path: Path):
        self.root = root_path.resolve()
        self.anchor_dir = (self.root / ".codigest" / "anchor").resolve()
        self.git_dir = (self.anchor_dir / ".shadow_git").resolve()

    def has_history(self) -> bool:
        """Checks if a valid anchor (git repo with commits) exists."""
        return self.git_dir.exists() and (self.git_dir / "HEAD").exists()

    def _run_git(self, args: list[str], cwd: Path | None = None, check=True) -> str:
        self.anchor_dir.mkdir(parents=True, exist_ok=True)
        base_cmd = [
            "git", 
            "--git-dir", str(self.git_dir), 
            "--work-tree", str(self.anchor_dir)
        ]

        cmd = base_cmd + args

        target_dir = cwd or self.anchor_dir
        
        result = subprocess.run(
            cmd, cwd=target_dir, capture_output=True, text=True, encoding='utf-8', errors='replace'
        )
        
        if check and result.returncode != 0:

            logger.debug(f"Shadow Git Warning ({args[0]}): {result.stderr.strip()}")
            
        return (result.stdout or "").strip()

    def has_changes(self, current_files: list[Path]) -> bool:
        """Fast check to determine if any file was modified/added/deleted since last anchor baseline."""
        if not self.git_dir.exists() or not (self.git_dir / "HEAD").exists() or not self.anchor_dir.exists():
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
                if src_stat.st_size != dest_stat.st_size or (src_stat.st_mtime_ns - dest_stat.st_mtime_ns) > 1_000_000:
                    return True
            except Exception:
                return True

        for anchor_file in self.anchor_dir.rglob("*"):
            if anchor_file.is_file() and ".shadow_git" not in anchor_file.parts and anchor_file.name != ".gitignore":
                try:
                    rel = anchor_file.relative_to(self.anchor_dir)
                    if rel not in current_rel_paths:
                        return True
                except Exception:
                    continue

        return False

    def update(self, source_files: list[Path]):
        self.anchor_dir.mkdir(parents=True, exist_ok=True)
        (self.anchor_dir / ".gitignore").write_text(".shadow_git\n", encoding="utf-8")
        if not self.git_dir.exists() or not (self.git_dir / "HEAD").exists():
            if self.git_dir.exists():
                shutil.rmtree(self.git_dir)
            subprocess.run(
                ["git", "init", "--bare", str(self.git_dir)],
                capture_output=True, check=False
            )
            self._run_git(["config", "core.bare", "false"])
            self._run_git(["config", "user.email", "codigest@ai"])
            self._run_git(["config", "user.name", "Context Manager"])
            self._run_git(["config", "core.autocrlf", "false"])
            self._run_git(["config", "gc.auto", "0"])

        # Quick check: if nothing changed since last anchor, skip work
        if self.git_dir.exists() and (self.git_dir / "HEAD").exists() and not self.has_changes(source_files):
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

                    if src_stat.st_size == dest_stat.st_size and dest_stat.st_mtime_ns >= src_stat.st_mtime_ns:
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
        elif self._run_git(["diff-index", "--quiet", "HEAD", "--"], check=False) != "":
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
                shutil.rmtree(d)
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
                    "--git-dir", str(self.git_dir),  # [변경] .shadow_git 경로 사용
                    "--work-tree", str(temp_baseline), 
                    "checkout", "HEAD", "--", "."
                ],
                capture_output=True, check=False
            )

            self._prune_ignored_files(temp_baseline, current_rel_paths)

            result = subprocess.run(
                ["git", "diff", "--no-index", "--no-prefix", "temp_diff_baseline", "temp_diff_current"],
                cwd=self.root / ".codigest",
                capture_output=True, text=True, encoding='utf-8', errors='replace'
            )
            
            diff_text = (result.stdout or "")
            diff_text = diff_text.replace("temp_diff_baseline/", "").replace("temp_diff_current/", "")
            
            clean_lines = []
            skip = False
            for line in diff_text.splitlines():
                if line.startswith("diff --git"):
                    skip = any(x in line for x in [".shadow_git", ".codigest", ".git"])
                if not skip:
                    clean_lines.append(line)
            return "\n".join(clean_lines)

        finally:
            for d in [temp_current, temp_baseline]:
                if d.exists():
                    shutil.rmtree(d)

    def _prune_ignored_files(self, baseline_dir: Path, valid_rel_paths: set[Path]):
        for file_path in baseline_dir.rglob("*"):
            if file_path.is_file() and ".git" not in file_path.parts and ".codigest" not in file_path.parts and ".shadow_git" not in file_path.parts:
                try:
                    rel_path = file_path.relative_to(baseline_dir)
                    if rel_path in valid_rel_paths:
                        continue
                    
                    real_file = self.root / rel_path
                    if real_file.exists():
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
                "--git-dir", str(self.git_dir), # [변경]
                "--work-tree", str(self.anchor_dir),
                "show", f"HEAD:{git_path}"
            ],
            capture_output=True, text=True, encoding='utf-8', errors='replace'
        )
        
        if result.returncode != 0:
            return ""
            
        return result.stdout

    def get_changed_files(self, current_files: list[Path]) -> list[Path]:
        raw_diff = self.get_changes(current_files)
        paths = set()
        for line in raw_diff.splitlines():
            if line.startswith("diff --git"):
                parts = line.split()
                if len(parts) >= 4:
                    p = parts[-1]
                    if p.startswith("b/") or p.startswith("a/"): p = p[2:]
                    if not any(part in (".codigest", ".git", ".shadow_git") for part in Path(p).parts):
                        paths.add(self.root / p)
        return sorted(list(paths))
