"""
Prompt Management Module.
Templates are aligned flush-left to prevent indentation issues when injecting large code blocks.
"""
import tomllib
from pathlib import Path
from typing import Callable, Dict
from . import tags

# RenderFunction takes keyword arguments and returns a processed string
RenderFunc = Callable[..., str]

# 1. Snapshot Template (codigest scan)
def _default_snapshot(project_name: str, tree_structure: str, source_code: str, instruction: str = "") -> str:
    instruction_block = ""
    if instruction:
        safe_instruction = tags.escape_xml_value(instruction)
        instruction_block = tags.dedent(f"""
<user_instruction>
{safe_instruction}
</user_instruction>
""")

    # Note: source_code is assumed to be already formatted via tags.file()
    return tags.dedent(f"""
[SYSTEM: CODIGEST - INITIAL CONTEXT]
You are an expert AI developer. I am providing the full context of a project named "{project_name}".
Please digest this structure and code to build your internal mental model.
{instruction_block}

<project_root>
{project_name}
</project_root>

<project_structure>
{tree_structure}
</project_structure>

<source_code>
{source_code}
</source_code>
""")

# 2. Diff Template (codigest diff)
def _default_diff(project_name: str, context_message: str, diff_content: str, instruction: str = "") -> str:
    instruction_block = ""
    if instruction:
        safe_instruction = tags.escape_xml_value(instruction)
        instruction_block = tags.dedent(f"""
<user_instruction>
{safe_instruction}
</user_instruction>
""")

    # Escape raw diff content to prevent XML injection
    safe_diff = tags.escape_xml_value(diff_content)

    return tags.dedent(f"""
[SYSTEM: CODIGEST - INCREMENTAL UPDATE]
Here are the latest changes for the project "{project_name}".
{context_message}.
{instruction_block}

Focus on these modifications to update your context.

<git_diff>
{safe_diff}
</git_diff>
""")

# 3. SemDiff Template (codigest semdiff)
def _default_semdiff(project_name: str, context_message: str, semdiff_content: str, instruction: str = "") -> str:
    instruction_block = ""
    if instruction:
        safe_instruction = tags.escape_xml_value(instruction)
        instruction_block = tags.dedent(f"""
<user_instruction>
{safe_instruction}
</user_instruction>
""")

    return tags.dedent(f"""
[SYSTEM: CODIGEST - SEMANTIC ANALYSIS]
Structural changes detected in "{project_name}".
{context_message}.
{instruction_block}

This report focuses on High-Level Architecture changes (Classes, Functions, Signatures).

<semantic_diff>
{semdiff_content}
</semantic_diff>
""")

# 4. Digest Template (codigest digest)
def _default_digest(project_name: str, tree_structure: str, digest_content: str, instruction: str = "") -> str:
    instruction_block = ""
    if instruction:
        safe_instruction = tags.escape_xml_value(instruction)
        instruction_block = tags.dedent(f"""
<user_instruction>
{safe_instruction}
</user_instruction>
""")

    return tags.dedent(f"""
[SYSTEM: CODIGEST - ARCHITECTURE DIGEST]
Here is the high-level architecture of "{project_name}".
This contains ONLY definitions (Classes, Functions), no implementation details.
Use this to understand the project structure and relationships.
{instruction_block}

<project_structure>
{tree_structure}
</project_structure>

<code_digest>
{digest_content}
</code_digest>
""")


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
        if key in self.overrides:
            raw_template = self.overrides[key]
            # Safety: Automatically escape values when using user-defined templates
            safe_kwargs = {
                k: tags.escape_xml_value(v) for k, v in kwargs.items()
            }
            try:
                return raw_template.format(**safe_kwargs)
            except KeyError as e:
                return f"Error rendering template '{key}': Missing argument {e}"

        # 2. Use Default (Code-based t-string)
        if key in DEFAULT_RENDERERS:
            # Default functions handle their own escaping logic internally where needed
            return DEFAULT_RENDERERS[key](**kwargs)

        return f"Error: Prompt template '{key}' not found."

def get_engine(root_path: Path) -> PromptEngine:
    return PromptEngine(root_path)
