"""
Core Template Engine & XML Factory.
Centralizes XML escaping and block generation logic.
"""

import re
import textwrap

# The elements codigest itself wraps context in. Only a closing tag of one of these inside
# the content could end a block early; everything else is left exactly as written.
WRAPPERS = (
    "file",
    "source_code",
    "project_snapshot",
    "project_structure",
    "code_digest",
    "changes",
    "semantic_diff",
    "user_instruction",
    "context_pack",
    "omitted",
    "terminal_history",
    "command",
    "output",
    "note",
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


def file(path: str, content: str, status: str = "", lines: str = "") -> str:
    """A <file> block with the content verbatim (see escape_xml_value).
    `lines` ("40-60") marks a block that holds only those lines."""
    # A file's own trailing newline would otherwise show up as a blank line before </file>.
    safe_content = escape_xml_value(content).rstrip("\n")
    lines_attr = f' lines="{lines}"' if lines else ""

    # The f-string starts at column 0 so no indentation leaks into the output.
    return f"""<file path="{path}{status}"{lines_attr}>
{safe_content}
</file>"""
