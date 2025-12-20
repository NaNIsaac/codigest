"""
Dependency Resolver Module.
Analyzes Python AST to resolve local imports recursively.
"""
import ast
from pathlib import Path

class DependencyResolver:
    def __init__(self, root_path: Path):
        self.root_path = root_path.resolve()
        self.visited: set[Path] = set()

    def resolve(self, initial_files: list[Path]) -> list[Path]:
        """
        Starting from initial_files, find all local dependencies recursively.
        """
        queue = [f.resolve() for f in initial_files if f.suffix == ".py"]
        self.visited = set(queue)
        
        # Keep non-python files as is
        results = set(initial_files)

        while queue:
            current_file = queue.pop(0)
            
            try:
                dependencies = self._get_imports(current_file)
            except Exception:
                # Syntax error or read error during parsing
                continue

            for dep_path in dependencies:
                if dep_path not in self.visited:
                    self.visited.add(dep_path)
                    queue.append(dep_path)
                    results.add(dep_path)
        
        return sorted(list(results))

    def _get_imports(self, file_path: Path) -> set[Path]:
        """Parse AST and extract local import paths."""
        local_deps = set()
        
        try:
            content = file_path.read_text(encoding="utf-8")
            tree = ast.parse(content)
        except Exception:
            return local_deps

        for node in ast.walk(tree):
            # 1. import module_name
            if isinstance(node, ast.Import):
                for alias in node.names:
                    path = self._resolve_import_path(alias.name, 0, file_path)
                    if path:
                        local_deps.add(path)
            
            # 2. from module_name import ...
            elif isinstance(node, ast.ImportFrom):
                if node.module:
                    path = self._resolve_import_path(node.module, node.level, file_path)
                    if path:
                        local_deps.add(path)
                elif node.level > 0:
                    # from . import module (module name is None)
                    # This case is tricky without module name, often handled by specific logic
                    pass

        return local_deps

    def _resolve_import_path(self, module_name: str, level: int, current_file: Path) -> Path | None:
        """
        Convert module string (e.g., 'core.scanner') to file path.
        Returns Path only if it exists locally.
        """
        # 1. Handle Relative Imports (from . import ...)
        if level > 0:
            # level 1 is current dir, 2 is parent, etc.
            base_dir = current_file.parent
            for _ in range(level - 1):
                base_dir = base_dir.parent
            
            # Construct potential path parts
            parts = module_name.split(".") if module_name else []
            candidate_base = base_dir.joinpath(*parts)
        else:
            # 2. Handle Absolute Imports
            # Try from root
            parts = module_name.split(".")
            candidate_base = self.root_path.joinpath(*parts)

        # Check possibilities:
        # A. module.py
        candidate_py = candidate_base.with_suffix(".py")
        if candidate_py.exists() and candidate_py.is_file():
            return candidate_py

        # B. module/__init__.py
        candidate_init = candidate_base / "__init__.py"
        if candidate_init.exists() and candidate_init.is_file():
            return candidate_init

        return None

def resolve_dependencies(root_path: Path, files: list[Path]) -> list[Path]:
    resolver = DependencyResolver(root_path)
    return resolver.resolve(files)