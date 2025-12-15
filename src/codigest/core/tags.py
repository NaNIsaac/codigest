"""
Core Template Engine & XML Factory.
Centralizes XML escaping and block generation logic.
"""
import html
import textwrap
from typing import Any

def escape_xml_value(value: Any) -> str:
    """[XML] Escapes characters for XML safety."""
    raw_value = str(value)
    safe_value = html.escape(raw_value, quote=False)
    if "</" in safe_value:
        safe_value = safe_value.replace("</", "&lt;/")
    return safe_value

def dedent(template: str) -> str:
    """[Plain] Removes common leading whitespace."""
    if not isinstance(template, str):
        template = str(template)
    return textwrap.dedent(template.expandtabs(4)).strip()

def xml(template: str) -> str:
    """[XML] Wrapper for dedent (legacy support)."""
    return dedent(template)

# 파일 블록 생성기 (이스케이프 자동화)
def file(path: str, content: str, status: str = "") -> str:
    """
    Generates a safe <file> block.
    Automatically escapes the content to prevent XML injection.
    """
    safe_content = escape_xml_value(content)
    
    # f-string을 왼쪽 벽에 붙여 들여쓰기 문제 원천 차단
    return f"""<file path="{path}{status}">
{safe_content}
</file>"""