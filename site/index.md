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
      text: Command Reference
      link: /commands
    - theme: alt
      text: GitHub
      link: https://github.com/NaNIsaac/codigest

install:
  label: Install
  command: uv tool install codigest
  copy: Copy
  copied: Copied

demo:
  alt: cdg cap runs a failing pytest, then cdg pack bundles the failure, the test file it names and the changed file
  caption: A test fails, and one command collects what the model needs.

features:
  - title: Bundles code you pick
    details: The whole project, chosen files and folders, or the local files they import (Python, JS/TS), as one block with the file tree.
    link: /commands#cdg-scan
  - title: Shows what changed
    details: Text or function-level diffs since a baseline you set, without touching your Git history.
    link: /commands#cdg-mark
  - title: Brings in terminal output
    details: It records commands you run and can hand over a failing one together with the files its error points at.
    link: /terminal-capture
  - title: Says what it included
    details: Every bundle reports its size, and pack gives the reason for each file and names the ones it left out.
    link: /commands#cdg-pack
  - title: Keeps secrets out
    details: .env files and keys are never included, and tokens in code or output are masked.
    link: /security
  - title: Works with agents
    details: Claude Code, Cursor and other MCP clients can request the same output.
    link: /mcp
---

Codigest does not call any AI model; it only prepares what you send to one. Everything runs on your machine.

## See it work

The examples below are real output from a small project: `prices.py` with a failing test in `test_prices.py`. The project grows between examples: `prices.py` gains a `Cart` class and a `legacy/` folder with a MATLAB file appears.

### Share the project

```console
$ cdg scan -s
```

```xml
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
def apply_discount(price, percent):
    return price * (1 - percent)
</file>

<file path="test_prices.py">
from prices import apply_discount


def test_ten_percent():
    assert apply_discount(200, 10) == 180
</file>
</source_code>
</project_snapshot>
```

In a terminal the result goes to the clipboard. `-s` also prints it.

### Start with an overview

`digest` lists every function, class and top-level variable with the line it starts on and the first line of its docstring, and leaves the bodies out.

```console
$ cdg digest -s
```

```xml
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
├── .gitignore
├── legacy
│   └── discount.m
├── prices.py
└── test_prices.py
</project_structure>
<file path="legacy/discount.m">
1: function p = discount(price, percent)
       Price after taking off percent percent.
5: %% Apply
</file>
<file path="prices.py">
 1: TAX_RATE = 0.1
 4: def apply_discount(price: float, percent: float) -> float
        Price after taking off `percent` percent.
 9: class Cart
        Items and the discount that applies to all of them.
12:   def __init__(self, percent: float=0)
16:   def total(self) -> float
          Sum after discount and tax.
</file>
<file path="test_prices.py">
4: def test_ten_percent()
</file>
</code_digest>
```

### Hand over a failing command

```console
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
1 failed in 0.04s

$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

The pack holds the failing command with its output, your uncommitted Git changes, `test_prices.py` because the error points at its line 5, `prices.py` because the test calls it, and `legacy/discount.m` because you edited it. [Output Formats](/output-formats) shows the whole block.
