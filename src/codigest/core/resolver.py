"""
Dependency Resolver Module.
Finds the project files a file imports: Python through its AST, JS/TS through
relative import specifiers. `scan -r` follows them recursively, `pack` one hop.
"""
lazy import ast
import re
import sys
from collections import deque
from pathlib import Path

JS_SUFFIXES = (".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs")
# Relative specifiers only: `from './x'`, `import './x'`, `require('./x')`, `import('./x')`.
JS_IMPORT_RE = re.compile(r"""(?:\bfrom|\bimport|\brequire\(|\bimport\()\s*['"](\.{1,2}/[^'"]+)['"]""")


def js_imports(path: Path) -> set[Path]:
    """Files a JS/TS module imports by relative path, resolved the way bundlers and tsc do."""
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return set()
    found = set()
    for spec in JS_IMPORT_RE.findall(source):
        base = (path.parent / spec).resolve()
        # TS ESM code imports `./x.js` for a file that is `x.ts` on disk.
        stem = base.with_suffix("") if base.suffix in JS_SUFFIXES else base
        for candidate in (base, *(stem.with_name(stem.name + s) for s in JS_SUFFIXES),
                          *(base / f"index{s}" for s in JS_SUFFIXES)):
            if candidate.is_file():
                found.add(candidate)
                break
    return found


class DependencyResolver:
    def __init__(self, root_path: Path):
        self.root_path = root_path.resolve()
        self.visited: set[Path] = set()
        
        self.resolve_cache: dict[str, Path | None] = {}

        self.stdlib_names = self._get_stdlib_names()

    def _get_stdlib_names(self) -> frozenset[str]:
        """Get set of standard library module names."""
        return sys.stdlib_module_names

    def imports_of(self, file_path: Path) -> set[Path]:
        """Project files that `file_path` imports, for any supported language."""
        if file_path.suffix in (".py", ".pyi"):
            return self.get_imports(file_path)
        if file_path.suffix in JS_SUFFIXES:
            return js_imports(file_path)
        return set()

    def resolve(self, initial_files: list[Path]) -> list[Path]:
        queue = deque(f.resolve() for f in initial_files)
        self.visited = set(queue)
        results = set(initial_files)

        while queue:
            current_file = queue.popleft()
            try:
                dependencies = self.imports_of(current_file)
            except Exception:
                continue

            for dep_path in dependencies:
                if dep_path not in self.visited:
                    self.visited.add(dep_path)
                    queue.append(dep_path)
                    results.add(dep_path)
        
        return sorted(results)

    def get_imports(self, file_path: Path) -> set[Path]:
        local_deps: set[Path] = set()
        try:
            content = file_path.read_text(encoding="utf-8")
            tree = ast.parse(content)
        except Exception:
            return local_deps

        for node in ast.walk(tree):
            match node:
                case ast.Import(names=names):
                    for alias in names:
                        # Case: import my_module
                        if path := self._check_cache_and_resolve(alias.name, 0, file_path):
                            local_deps.add(path)

                case ast.ImportFrom(module=module, names=names, level=level):
                    module_name = module or ""

                    # Case: from . import module
                    if module_name:
                        if path := self._check_cache_and_resolve(module_name, level, file_path):
                            local_deps.add(path)

                    # Case: from my_pkg import my_module
                    for alias in names:
                        full_target = f"{module_name}.{alias.name}" if module_name else alias.name
                        if path := self._check_cache_and_resolve(full_target, level, file_path):
                            local_deps.add(path)

        return local_deps

    def _check_cache_and_resolve(self, module_name: str, level: int, current_file: Path) -> Path | None:
        """Wrapper to handle caching logic."""
        
        # 1. 표준 라이브러리면 즉시 탈락 (I/O 없음)
        if level == 0 and module_name.split(".")[0] in self.stdlib_names:
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

        if level > 0:
            # Relative Import
            base_dir = current_file.parent
            for _ in range(level - 1):
                base_dir = base_dir.parent
            candidates = [base_dir.joinpath(*parts)]
        else:
            # Absolute Import Priorities
            candidates = [
                current_file.parent.joinpath(*parts),       # 1. Sibling (같은 폴더)
                self.root_path.joinpath(*parts),            # 2. Project Root
                self.root_path.joinpath("src", *parts),     # 3. Project Root/src
            ]

        # Check existence
        for base in candidates:
            # Check for .py file, then package (__init__.py)
            for p in (base.with_suffix(".py"), base / "__init__.py"):
                if p.is_file():
                    return p.resolve()

        return None

def resolve_dependencies(root_path: Path, files: list[Path]) -> list[Path]:
    resolver = DependencyResolver(root_path)
    return resolver.resolve(files)