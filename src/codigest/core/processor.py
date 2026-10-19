from pathlib import Path
from loguru import logger


def read_file_content(path: Path, add_line_numbers: bool = True) -> str:
    """
    Reads file content safely. Returns an error message string if binary/unreadable.
    """
    try:
        content = path.read_text(encoding="utf-8")

        if not add_line_numbers:
            return content

        lines = content.splitlines()
        if not lines:
            return ""

        # 라인 넘버 패딩 계산 (100줄이면 3칸 확보)
        width = len(str(len(lines)))

        return "\n".join(
            f"{i:>{width}}: {line}"
            for i, line in enumerate(lines, start=1)
        )

    except UnicodeDecodeError:
        return "<<Binary or Non-UTF8 Content>>"
    except Exception as e:
        logger.warning(f"Error reading {path}: {e}")
        return f"<<Error: {e}>>"


def read_line_range(path: Path, lo: int, hi: int | None) -> tuple[str, str]:
    """Lines lo..hi (1-based, inclusive), always numbered, and the range actually shown.
    A range past the end is cut to the file; `hi=None` means the single line `lo`."""
    try:
        rows = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return "<<Binary or Non-UTF8 Content>>", ""
    hi = min(lo if hi is None else hi, len(rows))
    if lo > hi:
        return f"<<{path.name} has {len(rows)} lines>>", ""
    width = len(str(hi))
    body = "\n".join(f"{i:>{width}}: {rows[i - 1]}" for i in range(lo, hi + 1))
    return body, str(lo) if lo == hi else f"{lo}-{hi}"
