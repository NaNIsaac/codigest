"""
Common Logic & Project Context Manager.
Centralizes project discovery, config loading, scanning, and dependency resolution.
Facade pattern to simplify command implementations.
"""

lazy import os
lazy import shutil
lazy import stat
lazy import tomllib
from pathlib import Path
from rich.console import Console

# Core modules
from . import scanner, resolver
from .redact import is_secret_file

# Warnings go to stderr so they never end up inside piped context.
console = Console(stderr=True)

type Targets = list[Path] | Path


class ProjectContext:
    def __init__(self, targets: Targets | None = None, root: Path | None = None):

        if isinstance(targets, Path):
            targets = [targets]

        if root is not None:
            # A fixed root (a remote repository's files in a temporary folder): never look
            # above it for a project marker.
            self.start_path = self.root_path = root
        else:
            # Paths in different folders of the same project (aps/a.m Auto_run/b.m) share the
            # project you are in; starting from the first path alone made its folder the root.
            cwd = Path.cwd().resolve()
            resolved = [t.resolve() for t in targets or []]
            if not resolved or all(t.is_relative_to(cwd) for t in resolved):
                self.start_path = cwd
            else:
                self.start_path = Path(os.path.commonpath(resolved))

            self.root_path = self._find_project_root(self.start_path)
        self.withheld: list[Path] = []
        self.config_extensions, self.config_ignores = self._load_config_filters()

    def _find_project_root(self, start_path: Path) -> Path:
        """
        Locates the project root by looking for marker directories/files.
        """
        current = start_path if start_path.is_dir() else start_path.parent

        # Traverse up
        for parent in [current, *current.parents]:
            if (parent / ".codigest").exists():
                return parent
            if (parent / ".git").exists() or (parent / ".shadow_git").exists():
                return parent

        return start_path if start_path.is_dir() else start_path.parent

    def _load_config_filters(self) -> tuple[set[str] | None, list[str]]:
        config_path = self.root_path / ".codigest" / "config.toml"
        extensions: set[str] | None = None
        exclude_patterns: list[str] = []

        if config_path.exists():
            try:
                with open(config_path, "rb") as f:
                    data = tomllib.load(f)
                    filters = data.get("filter", {})
                    if ext_list := filters.get("extensions", []):
                        extensions = set(ext_list)
                    exclude_patterns = filters.get("exclude_patterns", [])
            except Exception:
                pass
        return extensions, exclude_patterns

    def get_target_files(
        self,
        targets: Targets | None = None,
        ignore_config: bool = False,
        resolve_deps: bool = False,
    ) -> list[Path]:
        if isinstance(targets, Path):
            targets = [targets]
        # Resolve first: a relative target would not compare with the absolute root_path.
        resolved_targets = [t.resolve() for t in targets or []]

        scan_scope = resolved_targets or None

        exts, ignores = (None, [])
        if not ignore_config:
            exts, ignores = self.config_extensions, self.config_ignores

        for p in scan_scope or []:
            if not p.is_relative_to(self.root_path):
                console.print(
                    f"[yellow][Warning] {p.name} is outside detected root {self.root_path}[/yellow]"
                )

        files = scanner.scan_project(
            self.root_path, extensions=exts, extra_ignores=ignores, include_paths=scan_scope
        )

        if resolve_deps:
            files = resolver.resolve_dependencies(self.root_path, files)

        # Never hand out credential files, but remember them so commands can say so.
        self.withheld = sorted({f for f in files if is_secret_file(f.name)} | set(self.withheld))
        return [f for f in files if not is_secret_file(f.name)]


def get_context(targets: Targets | None = None, root: Path | None = None) -> ProjectContext:
    return ProjectContext(targets, root)


def remove_tree(path: Path) -> None:
    """`shutil.rmtree` that also deletes read-only files (git objects on Windows)."""

    def make_writable_and_retry(func, target, _exc):
        os.chmod(target, stat.S_IWRITE)
        func(target)

    shutil.rmtree(path, onexc=make_writable_and_retry)


def hide_from_git(directory: Path) -> None:
    """Make Git ignore everything in `directory` (the way pytest and ruff caches do), so a
    project that never listed `.codigest/` in its .gitignore does not see it in `git status`."""
    directory.mkdir(parents=True, exist_ok=True)
    marker = directory / ".gitignore"
    if not marker.exists():
        marker.write_text("*\n", encoding="utf-8")
