"""
Output formats besides the XML blocks: Markdown for people and chats that render it,
JSON for scripts.
"""

import re
from enum import StrEnum


class Format(StrEnum):
    XML = "xml"
    MARKDOWN = "markdown"
    JSON = "json"


_LANGUAGES = {
    "py": "python",
    "pyi": "python",
    "rs": "rust",
    "js": "javascript",
    "mjs": "javascript",
    "cjs": "javascript",
    "jsx": "jsx",
    "ts": "typescript",
    "tsx": "tsx",
    "go": "go",
    "m": "matlab",
    "c": "c",
    "h": "c",
    "cpp": "cpp",
    "cc": "cpp",
    "hpp": "cpp",
    "cxx": "cpp",
    "java": "java",
    "sh": "bash",
    "bash": "bash",
    "ps1": "powershell",
    "psm1": "powershell",
    "json": "json",
    "toml": "toml",
    "yaml": "yaml",
    "yml": "yaml",
    "md": "markdown",
    "html": "html",
    "css": "css",
    "sql": "sql",
    "xml": "xml",
}
_ASCII_LOWER = str.maketrans("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz")


def fence(content: str) -> str:
    """A code fence longer than any run of backticks in `content`, so the content cannot
    close the block early."""
    longest = max(map(len, re.findall(r"`+", content)), default=0)
    return "`" * max(longest + 1, 3)


def language(file_name: str) -> str:
    """The info string most renderers know for a file's extension; empty when unknown."""
    _, dot, ext = file_name.rpartition(".")
    return _LANGUAGES.get(ext.translate(_ASCII_LOWER), "") if dot else ""


def fenced(content: str, file_name: str = "") -> str:
    """A fenced block with a language tag taken from a file name."""
    mark = fence(content)
    body = content.removesuffix("\n")
    return f"{mark}{language(file_name)}\n{body}\n{mark}"
