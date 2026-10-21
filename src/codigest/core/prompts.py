"""
Prompt Management Module.
Templates are aligned flush-left to prevent indentation issues when injecting large code blocks.
"""

lazy import html
lazy import tomllib
from collections.abc import Callable
from pathlib import Path
from . import tags

# RenderFunction takes keyword arguments and returns a processed string
type RenderFunc = Callable[..., str]


def _instruction_block(instruction: str) -> str:
    if not instruction:
        return ""
    return f"<user_instruction>\n{tags.escape_xml_value(instruction)}\n</user_instruction>"


def _block(root: str, attrs: dict[str, str], instruction: str, *parts: str) -> str:
    """One root element holding the context. No canned instructions to the model:
    what to do with it is the user's `-m` (or their own prompts.toml template)."""
    head = " ".join(f'{k}="{html.escape(v)}"' for k, v in attrs.items() if v)
    body = [b for b in (_instruction_block(instruction), *parts) if b]
    return f"<{root} {head}>\n" + "\n".join(body) + f"\n</{root}>"


# Note: source_code / digest_content arrive already formatted via tags.file().
def _default_snapshot(
    project_name: str, tree_structure: str, source_code: str, instruction: str = ""
) -> str:
    return _block(
        "project_snapshot",
        {"project": project_name},
        instruction,
        f"<project_structure>\n{tree_structure}\n</project_structure>",
        f"<source_code>\n{source_code}\n</source_code>",
    )


def _default_diff(
    project_name: str, context_message: str, diff_content: str, instruction: str = ""
) -> str:
    return _block(
        "changes",
        {"project": project_name, "source": context_message},
        instruction,
        tags.escape_xml_value(diff_content),
    )


def _default_semdiff(
    project_name: str, context_message: str, semdiff_content: str, instruction: str = ""
) -> str:
    return _block(
        "semantic_diff",
        {"project": project_name, "source": context_message},
        instruction,
        semdiff_content,
    )


def _default_digest(
    project_name: str, tree_structure: str, digest_content: str, instruction: str = ""
) -> str:
    return _block(
        "code_digest",
        {"project": project_name, "contents": "signatures and docstrings, no bodies"},
        instruction,
        f"<project_structure>\n{tree_structure}\n</project_structure>",
        digest_content,
    )


# Registry of default implementations
# Ensure ALL keys correspond to prompt_engine.render calls
DEFAULT_RENDERERS: dict[str, RenderFunc] = {
    "snapshot": _default_snapshot,
    "diff": _default_diff,
    "semdiff": _default_semdiff,
    "digest": _default_digest,
}


class PromptEngine:
    def __init__(self, root_path: Path):
        self.root_path = root_path
        self.overrides = self._load_overrides()

    def _load_overrides(self) -> dict[str, str]:
        """Loads raw template strings from .codigest/prompts.toml"""
        prompt_file = self.root_path / ".codigest" / "prompts.toml"
        if not prompt_file.exists():
            return {}

        try:
            with open(prompt_file, "rb") as f:
                data = tomllib.load(f)
                return data.get("prompts", {})
        except Exception:
            return {}

    def render(self, key: str, **kwargs) -> str:
        """
        Renders a prompt by key.
        Priority: TOML Override > Default t-string Function
        """
        # 1. Check for TOML Override (String-based)
        if (raw_template := self.overrides.get(key)) is not None:
            # Code blocks arrive already wrapped by tags.file(); escaping them again would
            # turn their own <file> tags into text.
            safe_kwargs = {
                **kwargs,
                "instruction": tags.escape_xml_value(kwargs.get("instruction", "")),
            }
            try:
                return raw_template.format(**safe_kwargs)
            except KeyError as e:
                return f"Error rendering template '{key}': Missing argument {e}"

        # 2. Use Default (Code-based t-string)
        if renderer := DEFAULT_RENDERERS.get(key):
            # Default functions handle their own escaping logic internally where needed
            return renderer(**kwargs)

        return f"Error: Prompt template '{key}' not found."


def get_engine(root_path: Path) -> PromptEngine:
    return PromptEngine(root_path)
