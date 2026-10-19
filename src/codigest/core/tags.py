"""
Core Template Engine & XML Factory.
Centralizes XML escaping and block generation logic.
"""
import re
import textwrap

# The elements codigest itself wraps context in. Only a closing tag of one of these inside
# the content could end a block early; everything else is left exactly as written.
WRAPPERS = (
    "file", "source_code", "project_snapshot", "project_structure", "code_digest",
    "changes", "semantic_diff", "user_instruction", "context_pack", "omitted",
    "terminal_history", "command", "output", "note",
)
_CLOSING = re.compile(rf"</({'|'.join(WRAPPERS)})>")


def escape_xml_value(value: object) -> str:
    """Code goes in verbatim: `a -> b` must reach the reader as `a -> b`, not `a -&gt; b`,
    or it comes back escaped in the edits a model suggests. Only closing tags that would end
    one of codigest's own blocks are neutralised."""
    return _CLOSING.sub(r"&lt;/\1>", str(value))

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
    """A <file> block with the content verbatim (see escape_xml_value)."""
    # A file's own trailing newline would otherwise show up as a blank line before </file>.
    safe_content = escape_xml_value(content).rstrip("\n")

    # f-string을 왼쪽 벽에 붙여 들여쓰기 문제 원천 차단
    return f"""<file path="{path}{status}">
{safe_content}
</file>"""