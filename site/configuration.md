# Configuration

Codigest works without any configuration. Two optional files in a project's `.codigest/` folder change its behavior:

| File | Purpose |
| --- | --- |
| `.codigest/config.toml` | Which files every command considers. |
| `.codigest/prompts.toml` | Your own output templates for `scan`, `diff`, `semdiff` and `digest`. |

`cdg init` writes a starting `config.toml`. `prompts.toml` you create yourself.

The project root is the current folder or the nearest one above it that contains `.codigest/` or `.git/`. `.codigest/` is looked up there.

## config.toml

```toml
[filter]
# Only these extensions (omit to include every non-ignored file)
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# Extra exclusions, in .gitignore syntax (.gitignore itself is always honored)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]
```

### Keys that are applied

Two keys are read today, both under `[filter]`:

| Key | Effect |
| --- | --- |
| `extensions` | A list of file extensions to keep, each with its leading dot, in lower case (`".py"`). A file with any other extension is skipped, except `.gitignore`, `Dockerfile` and `pyproject.toml`, which are always kept. Leave the list out, or empty, to keep every file. |
| `exclude_patterns` | A list of patterns in `.gitignore` syntax, added to the project's `.gitignore` rules. |

The filters apply to every command that lists files: `scan`, `digest`, `tree`, `diff`, `semdiff`, `mark`, `status` and `pack`. A file you name on the command line is not removed by `extensions`. `scan -a`, `tree -a` ignore both keys for that run.

These are always skipped, whatever the config says: `.git/`, `.codigest/`, `.venv/`, `venv/`, `env/`, `__pycache__/`, `node_modules/`, `.idea/`, `.vscode/`, `*.pyc` and `*.DS_Store`. The `.gitignore` in the project root is always honored.

If `config.toml` cannot be parsed, it is ignored silently and no filter applies. Run `cdg status` to see the extensions Codigest read: it prints a `filters` line when the file exists.

### Keys that are not applied

`cdg init` also writes `[project]`, `max_file_size_kb` under `[filter]`, and an `[output]` section with `format` and `structure`. Nothing reads them today. Changing them has no effect, and you can delete them.

## prompts.toml

A template replaces the default wrapper of one command's output. Templates go under a `[prompts]` table, one key per command:

```toml
[prompts]
snapshot = """Review this code for bugs.
{instruction}
Project: {project_name}
{tree_structure}
{source_code}"""
```

The text is filled in with Python's `str.format`, so the placeholders are in braces and a literal brace is written twice (<code v-pre>{{</code> for <code v-pre>{</code>). The available names depend on the command:

| Key | Command | Placeholders |
| --- | --- | --- |
| `snapshot` | `scan` | `project_name`, `tree_structure`, `source_code`, `instruction` |
| `diff` | `diff` | `project_name`, `context_message`, `diff_content`, `instruction` |
| `semdiff` | `semdiff` | `project_name`, `context_message`, `semdiff_content`, `instruction` |
| `digest` | `digest` | `project_name`, `tree_structure`, `digest_content`, `instruction` |

`instruction` is what you pass with `-m`; it is empty without it. `source_code` and `digest_content` already hold finished `<file>` blocks. A key that is not in the table keeps its default.

With the template above, `cdg scan -m "Why does the test fail?"` prints:

```text
Review this code for bugs.
Why does the test fail?
Project: project
├── .gitignore
├── prices.py
└── test_prices.py
<file path=".gitignore">
...
```

When a template names a placeholder that does not exist, the output is an error message in place of the content. `pack`, `term` and `cap` do not use templates. Unlike the defaults, a template is not wrapped in an XML root element, so what you write is all there is.

The code in the blocks is passed verbatim; only `instruction` is XML-escaped.
