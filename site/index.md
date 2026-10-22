---
layout: home
title: Codigest
titleTemplate: Share the code context you choose with an AI

hero:
  name: Codigest
  text: Share the code context you choose with an AI
  tagline: Your project, a few files, what changed, or the command that failed. One block you can paste into any chat.
  actions:
    - theme: brand
      text: Get Started
      link: /getting-started
    - theme: alt
      text: GitHub
      link: https://github.com/NaNIsaac/codigest

install:
  label: Install Codigest
  windowsLabel: Windows
  unixLabel: Linux / macOS / Termux
  pythonLabel: Python (uv)
  windows: 'irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex'
  unix: 'curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh'
  python: 'uv tool install codigest'
  copy: Copy
  copied: Copied
  moreLabel: All installation options
  moreLink: /installation

demo:
  alt: cdg cap runs a failing pytest, then cdg pack bundles the failure, your uncommitted change and the two files involved
  caption: A test fails, and one command collects what the model needs.

workflowsMore: How it works
workflows:
  - title: Share the whole project
    details: The file tree and every file, minus what .gitignore and your config leave out, copied as one block.
    commands: ['cdg scan']
    link: /commands#cdg-scan
  - title: Share a few files and what they import
    details: Name the files or folders. -r adds the local files they import (Python, JS/TS).
    commands: ['cdg scan src/billing/invoice.py -r']
    link: /recipes#share-one-folder-or-one-question-s-worth-of-files
  - title: Share only what changed
    details: Set a baseline before you start, then send the text diff or the Python functions that changed. Git history is not touched.
    commands: ['cdg mark', 'cdg diff']
    link: /recipes#review-a-refactor-with-mark-and-semdiff
  - title: Hand over a failing test
    details: Run the test through cap, then pack the failure with the files its error points at and your changes.
    commands: ['cdg cap -- pytest -q', 'cdg pack']
    link: /recipes#a-test-fails-and-you-do-not-know-why
  - title: Let an agent ask for it
    details: Claude Code, Cursor and other MCP clients call the same commands and get the same output.
    commands: ['claude mcp add --scope user codigest -- cdg mcp']
    link: /mcp
---

Codigest does not call any AI model; it only prepares what you send to one. Everything runs on your machine.

## See it work

Real output from one run on a small project. You just edited `prices.py`, and the test in `test_prices.py` now fails.

::: code-group

```txt [cdg scan]
$ cdg scan -s
Copied snapshot to clipboard
<project_snapshot project="project">
<project_structure>
├── .gitignore
├── prices.py
└── test_prices.py
</project_structure>
<source_code>
<file path=".gitignore">
.venv/
__pycache__/
.pytest_cache/
</file>

<file path="prices.py">
"""Prices after discount and tax."""

TAX_RATE = 0.1


def apply_discount(price: float, percent: float) -> float:
    """Price after taking off `percent` percent."""
    return price * (1 - percent)


class Cart:
    """Items and the discount that applies to all of them."""

    def __init__(self, percent: float = 0):
        self.items: list[float] = []
        self.percent = percent

    def total(self) -> float:
        """Sum after discount and tax."""
        return sum(apply_discount(p, self.percent) for p in self.items) * (1 + TAX_RATE)
</file>

<file path="test_prices.py">
from prices import apply_discount


def test_ten_percent():
    assert apply_discount(200, 10) == 180
</file>
</source_code>
</project_snapshot>
```

```txt [cdg digest]
$ cdg digest -s
Digest 2 files, ~160 tokens
Copied digest to clipboard
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
├── .gitignore
├── prices.py
└── test_prices.py
</project_structure>
<file path="prices.py">
 1: """Prices after discount and tax."""
 3: TAX_RATE = 0.1
 6: def apply_discount(price: float, percent: float) -> float
        Price after taking off `percent` percent.
11: class Cart
        Items and the discount that applies to all of them.
14:   def __init__(self, percent: float=0)
18:   def total(self) -> float
          Sum after discount and tax.
</file>
<file path="test_prices.py">
4: def test_ten_percent()
</file>
</code_digest>
```

```txt [cdg cap]
$ cdg cap -- pytest -q
F                                                                        [100%]
================================== FAILURES ===================================
______________________________ test_ten_percent _______________________________

    def test_ten_percent():
>       assert apply_discount(200, 10) == 180
E       assert -1800 == 180
E        +  where -1800 = apply_discount(200, 10)

test_prices.py:5: AssertionError
=========================== short test summary info ===========================
FAILED test_prices.py::test_ten_percent - assert -1800 == 180
1 failed in 0.06s
```

```txt [cdg pack]
$ cdg pack
Packed ~545 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       called from failing test test_prices.py
Copied pack to clipboard
```

:::

- **scan** puts the project in one block. In a terminal it goes to the clipboard; `-s` also prints it.
- **digest** keeps the line number, signature and first docstring line of each definition and leaves the bodies out, so the model can ask for `apply_discount` by name.
- **cap** runs the test as usual and keeps its output.
- **pack** sends the failure, your uncommitted change, `test_prices.py` because the error points at its line 5, and `prices.py` because the test calls it. [Output Formats](/output-formats) shows a whole pack.

The output above is from the native `cdg` 0.12.2 on Windows. The Python package prints the same blocks; [Rust Port](/rust-port#differences-from-the-python-build) lists where the two differ.

## Common workflows

<HomeWorkflows />

## What it shares, and what it keeps out

<div class="home-grid">
<div>

### Stays on your machine

Codigest makes no network requests. Context leaves only when you paste it, or when an MCP client you connected asks for it.

</div>
<div>

### Never included

`.env` files, keys and certificates (`*.pem`, `*.key`, ...), SSH private keys, `.netrc` and `.pgpass`. Commands name the files they held back.

</div>
<div>

### Masked

Tokens with a known shape, such as GitHub, AWS, OpenAI or Slack keys, are replaced with `[REDACTED]`. Terminal captures are masked before they are stored.

</div>
<div>

### Not a guarantee

A secret in a format the rules do not know is not caught. Read what you paste. [Security](/security) lists every rule and its limits.

</div>
</div>
