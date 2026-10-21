# Output Formats

Every command prints one block of XML-like text. The tags give a model clear boundaries between the project tree, each file, a diff and a terminal run. Codigest adds no instructions of its own: what the model should do with the block is whatever you put in with `-m`, or in your own [template](/configuration#prompts-toml).

| Command | Root element |
| --- | --- |
| `scan` | `<project_snapshot>` |
| `digest` | `<code_digest>` |
| `diff` | `<changes>` |
| `semdiff` | `<semantic_diff>` |
| `pack` | `<context_pack>` |
| `term` | `<terminal_history>` |
| `tree` | plain text, no tags |

## Your instruction

`-m "..."` adds a `<user_instruction>` block as the first child of the root.

```xml
<project_snapshot project="project">
<user_instruction>
Why does the test fail?
</user_instruction>
<project_structure>
...
```

## Snapshot: scan

A `<project_structure>` with the file tree, then a `<source_code>` with one `<file>` per file. The `path` is relative to the project root and uses forward slashes.

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

A file outside the project root, pulled in by `-r`, is shown as `[EXTERNAL]/name`.

## Verbatim content

The code inside a `<file>` block is exactly as it is in the file. `->`, `<` and `&` are not escaped, so code copied out of a block can be pasted back, and edits a model suggests do not come back with `-&gt;` in them.

The one exception is a closing tag of one of Codigest's own blocks (`file`, `source_code`, `project_snapshot`, `project_structure`, `code_digest`, `changes`, `semantic_diff`, `user_instruction`, `context_pack`, `omitted`, `terminal_history`, `command`, `output`, `note`) appearing in the content. It is written with `&lt;` so the content cannot end the block early:

```xml
<file path="html.py">
def parse(s: str) -> dict[str, int]:
    return {k: 1 for k in s if k < "z" and k > "a"}

TEMPLATE = "&lt;/file>"
</file>
```

The file held `"</file>"`; the comparison operators and `->` are untouched.

## Line numbers and ranges

`scan -l` numbers every line. Numbers are right-aligned to the width of the last line number, followed by a colon and a space.

A range target (`scan prices.py:16-19`) numbers the lines it shows and adds a `lines` attribute, so the reader knows the block is a part:

```xml
<file path="prices.py" lines="16-19">
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
```

`pack` marks excerpts the same way. A long file that is only named in a failing output comes as the ten lines around each place the output points at, with `...` between separate gaps. Here the output said `pipeline.py:50`:

```xml
<file path="pipeline.py" reason="line 50 in failing output" lines="40-60">
 40: def step_14(x):
 41:     return x + 14
 42: 
 43: def step_15(x):
 44:     return x + 15
 45: 
 46: def step_16(x):
 47:     return x + 16
 48: 
 49: def step_17(x):
 50:     return x + 17
 51: 
 52: def step_18(x):
 53:     return x + 18
 54: 
 55: def step_19(x):
 56:     return x + 19
 57: 
 58: def step_20(x):
 59:     return x + 20
 60: 
</file>
```

The `digest` numbers are the lines where each definition starts.

## Digest: digest

```xml
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
...
</project_structure>
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
</code_digest>
```

Entries are in source order. Methods are indented under their class, and a docstring's first line sits under its entry.

## Changes: diff and semdiff

`diff` wraps a unified diff in `<changes>`. The `source` attribute says what it was compared against.

```xml
<changes project="project" source="since cdg mark (1 second ago)">
diff --git a/prices.py b/prices.py
...
</changes>
```

`semdiff` lists symbols per file with a marker and the line where the symbol starts:

```xml
<semantic_diff project="project" source="since cdg mark (1 second ago)">
<file path="prices.py">
[SIGNATURE] 16: method Cart.total :: (self) -> float -> (self, tax: bool=True) -> float
[LOGIC]     4: function apply_discount
[ADDED]     22: function tax_of
</file>
</semantic_diff>
```

## Terminal history: term

One `<command>` per capture. `index="1"` is the most recent.

```xml
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.4s" at="2026-10-21T23:30:51+09:00">
<output>
F                                                                        [100%]
...
1 failed in 0.04s
</output>
</command>
</terminal_history>
```

| Attribute | Meaning |
| --- | --- |
| `index` | 1 is the latest command, 3 is three commands ago. |
| `command` | The command line. |
| `exit_code` | Present when known. |
| `cwd` | The directory it ran in. |
| `hostname`, `shell` | Only with `cdg term --context`. |
| `duration` | Shown as `ms` below one second, otherwise seconds. |
| `at` | Start time with the UTC offset. |
| `truncated="true"` | The output was cut to 64 KB; the middle is replaced by a marker line. |
| `output_available="false"` | The command came from shell history and has no output. |

A `<note>` inside the command appears when the output was withheld on purpose, for example for a command that may print credentials. When nothing was captured, the block is `<terminal_history />`.

## Pack: pack

A pack puts everything in a fixed order: your instruction, the failed command, the diff, then the files, then an `<omitted>` list if anything did not fit.

```xml
<context_pack project="project">
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.4s" at="2026-10-21T23:30:51+09:00">
<output>
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
</output>
</command>
</terminal_history>
<changes source="uncommitted changes (git)">
diff --git a/prices.py b/prices.py
index 3d691df..9c00411 100644
--- a/prices.py
+++ b/prices.py
@@ -1,2 +1,19 @@
-def apply_discount(price, percent):
-    return price * (1 - percent / 100)
+TAX_RATE = 0.1
+
+
+def apply_discount(price: float, percent: float) -> float:
+    """Price after taking off `percent` percent."""
+    return price * (1 - percent)
+
+
+class Cart:
+    """Items and the discount that applies to all of them."""
+
+    def __init__(self, percent: float = 0):
+        self.items = []
+        self.percent = percent
+
+    def total(self) -> float:
+        """Sum after discount and tax."""
+        subtotal = sum(apply_discount(p, self.percent) for p in self.items)
+        return subtotal * (1 + TAX_RATE)
</changes>
<file path="test_prices.py" reason="line 5 in failing output">
1: from prices import apply_discount
2: 
3: 
4: def test_ten_percent():
5:     assert apply_discount(200, 10) == 180
</file>
<file path="prices.py" reason="called from failing test test_prices.py">
 1: TAX_RATE = 0.1
 2: 
 3: 
 4: def apply_discount(price: float, percent: float) -> float:
 5:     """Price after taking off `percent` percent."""
 6:     return price * (1 - percent)
 7: 
 8: 
 9: class Cart:
10:     """Items and the discount that applies to all of them."""
11: 
12:     def __init__(self, percent: float = 0):
13:         self.items = []
14:         self.percent = percent
15: 
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
<file path="legacy/discount.m" reason="changed">
1: function p = discount(price, percent)
2: % Price after taking off percent percent.
3: p = price * (1 - percent / 100);
4: 
5: %% Apply
</file>
</context_pack>
```

Each `<file>` carries a `reason`: `requested`, `changed`, `line 5 in failing output`, `called from failing test ...`, `tests prices.py`, or `imported by ...`. Files in a pack are always numbered.

Files that do not fit the budget, and credential files you named, are listed by name with the reason instead of being cut off silently. This is `cdg pack small.py notes.py .env -b 1000`:

```xml
<context_pack project="ex">
<file path="small.py" reason="requested">
1: VALUE = 1
</file>
<omitted>
.env: may contain secrets
notes.py: over budget (~3,170 tokens)
</omitted>
</context_pack>
```

## Piped and terminal output

| | In a terminal | Piped or redirected |
| --- | --- | --- |
| The block | Copied to the clipboard; printed only with `-s`. | Printed on stdout. |
| Status lines | Printed with the summary (sizes, which files). | Printed on stderr, so a pipe sees only the block. |
| Clipboard | Used. | Left alone, unless `--copy`. |
| Progress spinner | Shown while working. | Not shown. |

This is why `cdg scan > ctx.xml` yields a clean file, and why an agent's shell gets the text without anything touching your clipboard. `-o FILE` writes the block to a file in either case, with a trailing newline.

A large `scan` with nobody to answer the size question stops with a hint instead of waiting; `-y` takes it all.
