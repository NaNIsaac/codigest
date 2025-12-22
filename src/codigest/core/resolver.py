"""
Dependency Resolver Module.
Analyzes Python AST to resolve local imports recursively.
Optimized: Stdlib check -> Cache -> Local File Check.
"""
import ast
import sys
from pathlib import Path

class DependencyResolver:
    def __init__(self, root_path: Path):
        self.root_path = root_path.resolve()
        self.visited: set[Path] = set()
        
        self.resolve_cache: dict[str, Path | None] = {}

        self.stdlib_names = self._get_stdlib_names()

    def _get_stdlib_names(self) -> set[str]:
        """Get set of standard library module names."""
        if sys.version_info >= (3, 10):
            return sys.stdlib_module_names
        else:
            # Fallback for older python
            return set(sys.builtin_module_names) | {
                "os", "sys", "json", "pathlib", "typing", "subprocess", 
                "math", "random", "time", "datetime", "abc", "collections",
                "re", "shutil", "argparse", "logging", "unittest"
            }

    def resolve(self, initial_files: list[Path]) -> list[Path]:
        queue = [f.resolve() for f in initial_files if f.suffix == ".py"]
        self.visited = set(queue)
        results = set(initial_files)

        while queue:
            current_file = queue.pop(0)
            try:
                dependencies = self._get_imports(current_file)
            except Exception:
                continue

            for dep_path in dependencies:
                if dep_path not in self.visited:
                    self.visited.add(dep_path)
                    queue.append(dep_path)
                    results.add(dep_path)
        
        return sorted(list(results))

    def _get_imports(self, file_path: Path) -> set[Path]:
        local_deps = set()
        try:
            content = file_path.read_text(encoding="utf-8")
            tree = ast.parse(content)
        except Exception:
            return local_deps

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    # Case: import my_module
                    path = self._check_cache_and_resolve(alias.name, 0, file_path)
                    if path:
                        local_deps.add(path)
            
            elif isinstance(node, ast.ImportFrom):
                module_name = node.module if node.module else ""
                level = node.level

                # Case: from . import module
                if module_name:
                    path = self._check_cache_and_resolve(module_name, level, file_path)
                    if path:
                        local_deps.add(path)

                # Case: from my_pkg import my_module
                for alias in node.names:
                    full_target = f"{module_name}.{alias.name}" if module_name else alias.name
                    path = self._check_cache_and_resolve(full_target, level, file_path)
                    if path:
                        local_deps.add(path)

        return local_deps

    def _check_cache_and_resolve(self, module_name: str, level: int, current_file: Path) -> Path | None:
        """Wrapper to handle caching logic."""
        
        # 1. 표준 라이브러리면 즉시 탈락 (I/O 없음)
        if level == 0:
            root_pkg = module_name.split(".")[0]
            if root_pkg in self.stdlib_names:
                return None

        # 2. 상대 경로(level > 0)는 문맥(current_file)에 따라 달라지므로 캐싱 불가 -> 직접 해결
        if level > 0:
            return self._resolve_import_path(module_name, level, current_file)
        
        # 3. 절대 경로(import A.B)는 캐시 확인
        if module_name in self.resolve_cache:
            return self.resolve_cache[module_name]
        
        # 4. 캐시에 없으면 파일 시스템 조회 수행
        result = self._resolve_import_path(module_name, level, current_file)
        
        # 5. 결과 캐싱 (찾았든 못 찾았든)
        self.resolve_cache[module_name] = result
        return result

    def _resolve_import_path(self, module_name: str, level: int, current_file: Path) -> Path | None:
        """
        Physical file system check.
        """
        if not module_name:
            return None

        parts = module_name.split(".")
        candidates = []

        if level > 0:
            # Relative Import
            base_dir = current_file.parent
            for _ in range(level - 1):
                base_dir = base_dir.parent
            candidates.append(base_dir.joinpath(*parts))
        else:
            # Absolute Import Priorities
            # 1. Sibling (같은 폴더)
            candidates.append(current_file.parent.joinpath(*parts))
            # 2. Project Root
            candidates.append(self.root_path.joinpath(*parts))
            # 3. Project Root/src
            candidates.append(self.root_path.joinpath("src", *parts))

        # Check existence
        for base in candidates:
            # Check for .py file
            p_py = base.with_suffix(".py")
            if p_py.is_file():
                return p_py.resolve()
            
            # Check for package (__init__.py)
            p_init = base / "__init__.py"
            if p_init.is_file():
                return p_init.resolve()

        return None

def resolve_dependencies(root_path: Path, files: list[Path]) -> list[Path]:
    resolver = DependencyResolver(root_path)
    return resolver.resolve(files)