"""
Common Logic & Project Context Manager.
Centralizes project discovery, config loading, scanning, and dependency resolution.
Facade pattern to simplify command implementations.
"""
import tomllib
from pathlib import Path
from typing import Tuple, List, Optional, Set, Union
from rich.console import Console

# Core modules
from . import scanner, resolver

console = Console()

class ProjectContext:
    # [수정] targets 타입을 Union[list[Path], Path]로 확장
    def __init__(self, targets: Optional[Union[list[Path], Path]] = None):
        
        # [추가] 단일 Path가 들어오면 리스트로 감싸기 (Safety)
        if isinstance(targets, Path):
            targets = [targets]

        # 1. 탐색 시작점 결정
        if targets and len(targets) > 0:
            self.start_path = targets[0].resolve()
        else:
            self.start_path = Path.cwd().resolve()
            
        # 2. 루트 찾기
        self.root_path = self._find_project_root(self.start_path)
        self.config_extensions, self.config_ignores = self._load_config_filters()

    def _find_project_root(self, start_path: Path) -> Path:
        """
        Locates the project root by looking for marker directories/files.
        """
        current = start_path if start_path.is_dir() else start_path.parent
        
        # Traverse up
        for parent in [current] + list(current.parents):
            if (parent / ".codigest").exists():
                return parent
            if (parent / ".git").exists() or (parent / ".shadow_git").exists():
                return parent
        
        return start_path if start_path.is_dir() else start_path.parent

    def _load_config_filters(self) -> Tuple[Optional[set[str]], list[str]]:
        config_path = self.root_path / ".codigest" / "config.toml"
        extensions = None
        exclude_patterns = []
        
        if config_path.exists():
            try:
                with open(config_path, "rb") as f:
                    data = tomllib.load(f)
                    filters = data.get("filter", {})
                    ext_list = filters.get("extensions", [])
                    if ext_list:
                        extensions = set(ext_list)
                    exclude_patterns = filters.get("exclude_patterns", [])
            except Exception:
                pass 
        return extensions, exclude_patterns

    def get_target_files(
        self, 
        targets: Optional[Union[list[Path], Path]] = None, 
        ignore_config: bool = False,
        resolve_deps: bool = False
    ) -> list[Path]:
        """
        The Master Method:
        """
        # [핵심 수정] 입력된 경로를 무조건 '절대 경로'로 변환 (Resolve)
        # 이걸 안 하면 'src' 같은 상대 경로 입력 시 root_path(절대 경로)와 비교 실패함
        resolved_targets = []
        if targets:
            # 단일 Path면 리스트로 변환
            if isinstance(targets, Path):
                targets = [targets]
            
            # 리스트 내부의 모든 경로를 resolve()
            for t in targets:
                resolved_targets.append(t.resolve())
        
        # 이후 로직에서는 변환된 resolved_targets를 사용
        scan_scope = resolved_targets if resolved_targets else None
        
        # 1. Determine Filters
        exts, ignores = (None, [])
        if not ignore_config:
            exts, ignores = self.config_extensions, self.config_ignores

        # 2. Scope Warning UI (이제 정상 작동함)
        if scan_scope:
            for p in scan_scope:
                if not p.is_relative_to(self.root_path):
                    console.print(f"[yellow][Warning] {p.name} is outside detected root {self.root_path}[/yellow]")

        # 3. Scan
        files = scanner.scan_project(
            self.root_path, 
            extensions=exts, 
            extra_ignores=ignores, 
            include_paths=scan_scope
        )

        # 4. Resolve Dependencies
        if resolve_deps:
            files = resolver.resolve_dependencies(self.root_path, files)

        return files

# Helper for quick access
def get_context(targets: Optional[Union[list[Path], Path]] = None) -> ProjectContext:
    return ProjectContext(targets)