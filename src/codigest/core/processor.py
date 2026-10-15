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