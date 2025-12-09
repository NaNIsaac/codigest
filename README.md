# Codigest

Codigest is a standalone CLI tool designed to extract, structure, and track the context of your codebase for Large Language Models (LLMs).

Unlike simple copy-paste tools, Codigest employs a **Context Anchor** system (Shadow Git) to track changes locally without polluting your main version control history. It also features **Semantic Analysis** to detect structural changes in your code.

## Core Philosophy

  * **Read-Only & Safe**: Codigest never modifies your source code. It only reads, analyzes, and formats context.
  * **Context-Aware**: Instead of dumping raw text, it structures code into XML snapshots designed for LLM comprehension.
  * **Session-Based Tracking**: It maintains an internal anchor to track "work-in-progress" changes independently of your Git commits.
  * **Environment Isolated**: Runs in its own isolated Python 3.14 environment via `uv`, ensuring it never conflicts with your project's dependencies.

## Installation

Codigest runs as a global tool. You don't need to add it to your project's `requirements.txt`.
We strongly recommend using **uv** for the best experience.

### Step 1: Install `uv`

If you don't have `uv` installed, use **Winget** (Windows) or curl (macOS/Linux).

```powershell
# Windows (via Winget)
winget install --id astral-sh.uv

# macOS/Linux
curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Step 2: Install `codigest`

Install Codigest globally. `uv` will automatically manage the required Python 3.14 environment for you.

```bash
uv tool install codigest
```

### Step 3: Update Path (Important\!)

To run `codigest` from any terminal, ensure your tools directory is in your PATH.

```bash
uv tool update-shell
```

*\> **Note:** Restart your terminal (or VS Code) after this step to apply changes.*

-----

## Workflow & Commands

Once installed, you can use `codigest` command anywhere.

### 1\. Initialization

Sets up the `.codigest` directory and captures the initial baseline anchor. 

```bash
codigest init
```

### 2\. Full Context Snapshot (`scan`)

Scans the entire codebase and generates a structured XML snapshot.

  * **Output:** `.codigest/snapshot.xml`
  * **Use Case:** Providing the LLM with the full project context at the start of a session.
  * **Side Effect:** Updates the internal Context Anchor to the current state.

```bash
codigest scan --message "Initial project context"
```

### 3\. Incremental Changes (`diff`)

Tracks text-based changes between the last `scan` and the current working tree.

  * **Output:** `.codigest/changes.diff`
  * **Use Case:** "I modified 3 files. Here is exactly what changed since the last snapshot."
  * **Note:** This is based on the internal Shadow Git, not your project's Git history.


```bash
codigest diff
```

### 4\. Semantic Analysis (`semdiff`)

Analyzes **structural changes** (AST-based) rather than line-by-line text differences.

  * **Output:** `.codigest/semdiff.xml`
  * **Use Case:** "I refactored the API. Show me which functions were added, removed, or had their signatures changed."
  * **Benefit:** Significantly reduces token usage compared to raw diffs by ignoring formatting/comment changes.


```bash
codigest semdiff
```

### 5\. Architecture Digest (`digest`)

Generates a high-level outline of the project structure (Classes, Functions, Methods only).

  * **Output:** `.codigest/digest.xml`
  * **Use Case:** "Don't read the implementation details. Just understand the class hierarchy and available methods."


```bash
codigest digest
```

-----

## Configuration

You can customize behavior in `.codigest/config.toml`.

```toml
[filter]
# Target extensions
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# Exclude patterns (Gitignore syntax)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]

[output]
format = "xml"
```

## Architecture Details

**Context Anchor (Shadow Git)**
Codigest maintains a hidden, lightweight Git repository inside `.codigest/anchor`.

  * When you run `scan`, the current state is committed to this anchor.
  * When you run `diff`, the tool compares your working directory against this anchor.
  * When you run `semdiff`, the tool parses the AST of the anchor version and the current version to detect logical drifts.

**Safety Mechanisms**

  * **structure-aware dedent:** Ensures XML tags are perfectly aligned regardless of code indentation.
  * **Automatic Exclusion:** Self-referential files (`.codigest/`) are automatically ignored to prevent recursion loops.

## License

MIT License