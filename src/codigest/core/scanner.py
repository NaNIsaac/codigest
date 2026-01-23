"""
Core File System Scanner.
"""
import pathspec
from pathlib import Path
from typing import Optional
from loguru import logger

# Default "Safe" ignores
ALWAYS_IGNORE = [
    ".git/", ".codigest/",
    ".venv/", "venv/", "env/",
    "__pycache__/", "node_modules/",
    ".idea/", ".vscode/",
    "*.pyc", "*.DS_Store"
]

class ProjectScanner:
    def __init__(
            self,
            root_path: Path,
            extensions: Optional[set[str]] = None,
            extra_ignores: Optional[list[str]] = None,
            include_paths: Optional[list[Path]] = None
            ):
        self.root_path = root_path
        self.extensions = extensions
        self.extra_ignores = extra_ignores or []
        self.ignore_spec = self._load_gitignore()
        self.include_paths = include_paths

    def _load_gitignore(self) -> pathspec.PathSpec:
        """Loads .gitignore and combines with ALWAYS_IGNORE and extra_ignores."""
        patterns = list(ALWAYS_IGNORE)
        
        # Add config-based ignores
        if self.extra_ignores:
            patterns.extend(self.extra_ignores)

        gitignore_path = self.root_path / ".gitignore"
        if gitignore_path.exists():
            try:
                with open(gitignore_path, "r", encoding="utf-8") as f:
                    user_patterns = [
                        line.strip() for line in f 
                        if line.strip() and not line.startswith("#")
                    ]
                    patterns.extend(user_patterns)
            except Exception as e:
                logger.warning(f"Failed to read .gitignore: {e}")
                
        return pathspec.PathSpec.from_lines("gitwildmatch", patterns)

    def is_ignored(self, path: Path) -> bool:
        """Checks if a path matches the ignore patterns."""
        try:
            # pathspec requires relative paths (e.g., "src/main.py")
            rel_path = path.relative_to(self.root_path).as_posix()
            if path.is_dir():
                rel_path += "/"
        except ValueError:
            return False
            
        return self.ignore_spec.match_file(rel_path)

    def _is_included(self, entry: Path) -> bool:
        """파일이 지정된 include_paths 범위 안에 있는지 확인"""
        if not self.include_paths:
            return True
            
        for path in self.include_paths:
            try:
                entry.relative_to(path)
                return True
            except ValueError:
                continue
        return False

    def scan(self) -> list[Path]:
        """
        Walks the directory tree and returns valid files.
        (Modernized from old core.py's stack-based approach)
        """
        valid_files = []
        start_dirs = self.include_paths if self.include_paths else [self.root_path]
        # Use rglob for simplicity, but we must manually filter dirs to respect gitignore
        # For better performance on large repos, we use a manual walk similar to the old code
        dirs_stack = []
        for p in start_dirs:
            if p.is_file():
                if not self.is_ignored(p):
                    valid_files.append(p)
            elif p.is_dir():
                dirs_stack.append(p)

        while dirs_stack:
            current = dirs_stack.pop()
            
            try:
                # Sort for deterministic output (LLMs like order)
                entries = sorted(current.iterdir(), key=lambda x: x.name)
            except PermissionError:
                logger.debug(f"Permission denied: {current}")
                continue
                
            for entry in entries:
                if self.is_ignored(entry):
                    continue
                
                if entry.is_dir():
                    dirs_stack.append(entry)

                elif entry.is_file():
                    # Extension Filter Check
                    if self.extensions and entry.suffix.lower() not in self.extensions:
                        # Allow specific config files even if extension doesn't match
                        if entry.name not in {".gitignore", "Dockerfile", "pyproject.toml"}:
                            continue
                            
                    valid_files.append(entry)

        return sorted(list(set(valid_files)))

def scan_project(
    root_path: Path, 
    extensions: set[str] | None = None, 
    extra_ignores: list[str] | None = None, 
    include_paths: list[Path] | None = None
) -> list[Path]:
    scanner = ProjectScanner(root_path, extensions, extra_ignores, include_paths)
    return scanner.scan()