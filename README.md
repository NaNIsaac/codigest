# Codigest

**Codigest** is a standalone CLI tool designed to extract, structure, and track the context of your codebase for Large Language Models (LLMs).

Unlike simple copy-paste tools, Codigest employs a **Context Anchor** system (Shadow Git) to track changes locally without polluting your main version control history. It also features **Semantic Analysis** to detect structural changes in your code.

> **Note:** You can use the short alias **`cdg`** instead of `codigest` for faster typing.

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
curl -LsSf [https://astral.sh/uv/install.sh](https://astral.sh/uv/install.sh) | sh

```

### Step 2: Install `codigest`

Install Codigest globally. `uv` will automatically manage the required Python 3.14 environment for you.

```bash
uv tool install codigest
```

### Step 3: Update Path (Important!)

To run `codigest` (or `cdg`) from any terminal, ensure your tools directory is in your PATH.

```bash
uv tool update-shell
```

*> **Note:** Restart your terminal (or VS Code) after this step to apply changes.*

---

## Workflow & Commands

Once installed, use the **`cdg`** command anywhere.

### 1. Initialization

Sets up the `.codigest` directory and captures the initial baseline anchor.

```bash
cdg init
```

### 2. Full Context Snapshot (`scan`)

Scans the codebase and generates a structured XML snapshot. Includes a **Pre-flight Check** to prevent accidental token overflow.

* **Output:** `.codigest/snapshot.xml`
* **Features:**
* **Smart Confirmation:** Automatically skips confirmation for small contexts, but warns you for large ones (>30k tokens).
* **Dependency Resolution (`-r`):** Automatically finds and includes local files imported by your target files.
* **Scope Control:** You can specify folders or files to scan.



```bash
# Basic scan (Interactive confirmation if large)
cdg scan

# Scan specific folder with dependency resolution (Smart Context)
cdg scan src/main.py -r

# Force execution without confirmation (Good for CI/CD)
cdg scan -y --message "Automated snapshot"
```

### 3. Incremental Changes (`diff`)

Tracks text-based changes between the last `scan` and the current working tree.

* **Output:** `.codigest/changes.diff`
* **Use Case:** "I modified 3 files. Here is exactly what changed since the last snapshot."
* **Note:** Checks against the internal Shadow Git, enabling tracking without committing to the real Git.

```bash
cdg diff
```

### 4. Semantic Analysis (`semdiff`)

Analyzes **structural changes** (AST-based) rather than line-by-line text differences.

* **Output:** `.codigest/semdiff.xml`
* **Use Case:** "I refactored the API. Show me added/removed functions or signature changes."
* **Benefit:** Reduces token usage by ignoring formatting/comment changes.

```bash
cdg semdiff
```

### 5. Project Tree (`tree`)

Visualizes the project structure. Respects `.gitignore`.

* **Style:** Professional, cleaner output without emojis for better readability.
* **Use Case:** Quickly verifying which files are being tracked.

```bash
cdg tree
```

### 6. Architecture Digest (`digest`)

Generates a high-level outline of the project structure (Classes, Functions, Methods only).

* **Output:** `.codigest/digest.xml`
* **Use Case:** "Don't read the implementation details. Just understand the class hierarchy."

```bash
cdg digest
```

---

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
Codigest maintains a hidden, lightweight Git repository inside `.codigest/anchor/.shadow_git`.

* It is renamed to `.shadow_git` to prevent VS Code and other IDEs from confusing it with your project's actual repository.
* When you run `scan`, the current state is committed to this anchor.
* When you run `diff`, the tool compares your working directory against this anchor.

**Safety Mechanisms**

* **Pre-flight Check:** Calculates estimated token count before processing to prevent context overflow errors.
* **Structure-Aware Dedent:** Ensures XML tags are perfectly aligned (flush-left) to prevent indentation artifacts in LLM prompts.
* **XML Injection Protection:** Automatically escapes content within XML tags.

## License

MIT License
