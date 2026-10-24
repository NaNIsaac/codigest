from pathlib import Path


def generate_toon(paths: list[Path], root_dir: Path) -> str:
    """Placeholder: TOON output is not implemented yet."""
    return "# TOON generation logic here"


def generate_ascii_tree(paths: list[Path], root_dir: Path, depth: int | None = None) -> str:
    """
    [Legacy Style] Generates a standard ASCII directory tree.
    Below `depth` levels a folder is shown once with the number of files under it.
    """
    lines = []
    path_set = set(paths)
    dirs_with_files = {parent for path in paths for parent in path.parents}

    def _walk(current_path: Path, prefix: str = "", level: int = 1):
        try:
            all_items = sorted(current_path.iterdir(), key=lambda x: x.name)
        except PermissionError:
            return

        filtered_items = []
        for item in all_items:
            if item.is_file():
                if item in path_set:
                    filtered_items.append(item)
            elif item.is_dir() and item in dirs_with_files:
                filtered_items.append(item)

        count = len(filtered_items)
        for i, item in enumerate(filtered_items):
            is_last = i == count - 1
            connector = "└── " if is_last else "├── "

            if item.is_dir() and depth is not None and level >= depth:
                under = sum(1 for p in paths if p.is_relative_to(item))
                lines.append(f"{prefix}{connector}{item.name}/ ({under} file{'s' * (under != 1)})")
                continue

            lines.append(f"{prefix}{connector}{item.name}")

            if item.is_dir():
                extension = "    " if is_last else "│   "
                _walk(item, prefix + extension, level + 1)

    _walk(root_dir)
    return "\n".join(lines)
